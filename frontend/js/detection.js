class ColorDetectionSystem {
    constructor() {
        // Configuration de l'URL de base pour l'API
        this.API_BASE_URL = window.location.origin; // Utilise automatiquement le bon port
        
        this.video = document.getElementById('video');
        this.canvas = document.getElementById('canvas');
        this.ctx = this.canvas?.getContext('2d');
        this.stream = null;
        this.isDetecting = false;
        this.detectionInterval = null;
        this.historyRefreshInterval = null;
        
        // Compteurs de cadence
        this.cadenceCounters = {
            red: { count: 0, startTime: Date.now() },
            green: { count: 0, startTime: Date.now() },
            blue: { count: 0, startTime: Date.now() }
        };
        
        // Configuration des couleurs HSV (optimisée)
        this.colors = {
            red: {
                ranges: [[0, 120, 70, 10, 255, 255], [170, 120, 70, 180, 255, 255]],
                label: "Carte microchip",
                gId: "1001"
            },
            green: {
                ranges: [[36, 50, 70, 89, 255, 255]],
                label: "Carte personnalisée", 
                gId: "1002"
            },
            blue: {
                ranges: [[90, 50, 70, 128, 255, 255]],
                label: "STM32",
                gId: "1003"
            }
        };
        
        // Dernières détections pour éviter les doublons
        this.lastDetectionTimes = {
            red: 0,
            green: 0,
            blue: 0
        };
        this.detectionCooldown = 3000; // 3 secondes entre détections
        
        this.initEventListeners();
        this.loadDetections();
        this.updateCadenceDisplay();
        this.startHistoryAutoRefresh();
    }

    initEventListeners() {
        const startButton = document.getElementById('startCamera');
        const stopButton = document.getElementById('stopCamera');
        const resetButton = document.getElementById('resetCadence');
        const refreshButton = document.getElementById('refreshDB');

        if (startButton) {
            startButton.addEventListener('click', () => this.startCamera());
        }
        if (stopButton) {
            stopButton.addEventListener('click', () => this.stopCamera());
        }
        if (resetButton) {
            resetButton.addEventListener('click', () => this.resetCadence());
        }
        if (refreshButton) {
            refreshButton.addEventListener('click', () => this.loadDetections());
        }
        
        // Gestion de la fermeture de la page
        window.addEventListener('beforeunload', () => {
            this.cleanup();
        });
    }

    async startCamera() {
        try {
            console.log('Démarrage de la caméra...');
            
            // Arrêter la caméra existante si elle existe
            this.stopCamera();
            
            // Configuration des contraintes média optimisée
            const constraints = {
                video: {
                    width: { ideal: 1280, min: 640 },
                    height: { ideal: 720, min: 480 },
                    facingMode: 'environment', // Caméra arrière si disponible
                    frameRate: { ideal: 30 }
                },
                audio: false
            };
            
            // Demander l'accès à la caméra
            this.stream = await navigator.mediaDevices.getUserMedia(constraints);
            console.log('Stream caméra obtenu');
            
            if (this.video) {
                this.video.srcObject = this.stream;
                
                // Attendre que les métadonnées soient chargées
                await new Promise((resolve, reject) => {
                    this.video.onloadedmetadata = () => {
                        console.log(`Caméra initialisée: ${this.video.videoWidth}x${this.video.videoHeight}`);
                        
                        // Configurer le canvas
                        if (this.canvas) {
                            this.canvas.width = this.video.videoWidth || 1280;
                            this.canvas.height = this.video.videoHeight || 720;
                            console.log(`Canvas configuré: ${this.canvas.width}x${this.canvas.height}`);
                        }
                        
                        resolve();
                    };
                    
                    this.video.onerror = (e) => {
                        console.error('Erreur vidéo:', e);
                        reject(e);
                    };
                    
                    // Timeout de sécurité
                    setTimeout(() => {
                        if (this.video.readyState < 1) {
                            reject(new Error('Timeout chargement vidéo'));
                        }
                    }, 10000);
                });
                
                // Démarrer la lecture vidéo
                await this.video.play();
                console.log('Lecture vidéo démarrée');
                
                // Démarrer la détection après un court délai
                setTimeout(() => {
                    this.startDetection();
                }, 1000);
            }
            
            this.updateStatus('Caméra démarrée - Détection active', 'success');
            this.updateButtonStates(true);
            
        } catch (error) {
            console.error('Erreur accès caméra:', error);
            let errorMessage = 'Erreur: Impossible d\'accéder à la caméra';
            
            if (error.name === 'NotAllowedError') {
                errorMessage = 'Accès à la caméra refusé. Veuillez autoriser l\'accès dans les paramètres du navigateur.';
            } else if (error.name === 'NotFoundError') {
                errorMessage = 'Aucune caméra trouvée sur cet appareil.';
            } else if (error.name === 'NotReadableError') {
                errorMessage = 'Caméra déjà utilisée par une autre application.';
            } else if (error.message) {
                errorMessage = `Erreur: ${error.message}`;
            }
            
            this.updateStatus(errorMessage, 'error');
        }
    }

    stopCamera() {
        console.log('Arrêt de la caméra...');
        
        if (this.stream) {
            this.stream.getTracks().forEach(track => {
                track.stop();
                console.log('Track arrêté:', track.kind);
            });
            this.stream = null;
        }
        
        if (this.video) {
            this.video.srcObject = null;
            this.video.pause();
        }
        
        this.stopDetection();
        this.updateStatus('Caméra arrêtée', 'info');
        this.updateButtonStates(false);
    }

    startDetection() {
        if (this.isDetecting) {
            console.log('Détection déjà en cours');
            return;
        }
        
        if (!this.video || !this.canvas || !this.ctx) {
            console.error('Éléments vidéo/canvas manquants');
            return;
        }
        
        if (this.video.readyState < 2) {
            console.log('Attente chargement vidéo...');
            setTimeout(() => this.startDetection(), 500);
            return;
        }
        
        console.log('Démarrage de la détection');
        this.isDetecting = true;
        
        this.detectionInterval = setInterval(() => {
            this.processFrame();
        }, 150); // Détection toutes les 150ms pour de meilleures performances
    }

    stopDetection() {
        console.log('Arrêt de la détection');
        this.isDetecting = false;
        if (this.detectionInterval) {
            clearInterval(this.detectionInterval);
            this.detectionInterval = null;
        }
    }

    processFrame() {
        if (!this.video?.videoWidth || !this.video?.videoHeight || !this.ctx) {
            return;
        }
        
        if (this.video.paused || this.video.ended) {
            return;
        }

        try {
            // Capturer le frame
            this.ctx.drawImage(this.video, 0, 0, this.canvas.width, this.canvas.height);
            const imageData = this.ctx.getImageData(0, 0, this.canvas.width, this.canvas.height);
            
            // Convertir en HSV et détecter les couleurs
            const hsv = this.rgbToHsv(imageData);

            // Détecter chaque couleur
            for (const [colorName, config] of Object.entries(this.colors)) {
                if (this.detectColor(hsv, config.ranges)) {
                    const now = Date.now();
                    if (now - this.lastDetectionTimes[colorName] > this.detectionCooldown) {
                        this.onColorDetected(colorName, config);
                        this.lastDetectionTimes[colorName] = now;
                    }
                }
            }
        } catch (error) {
            console.error('Erreur traitement frame:', error);
        }
    }

    rgbToHsv(imageData) {
        const data = imageData.data;
        const hsv = new Uint8Array(data.length);
        
        for (let i = 0; i < data.length; i += 4) {
            const r = data[i] / 255;
            const g = data[i + 1] / 255;
            const b = data[i + 2] / 255;
            
            const max = Math.max(r, g, b);
            const min = Math.min(r, g, b);
            const diff = max - min;
            
            let h = 0;
            if (diff !== 0) {
                if (max === r) {
                    h = ((g - b) / diff) % 6;
                } else if (max === g) {
                    h = (b - r) / diff + 2;
                } else {
                    h = (r - g) / diff + 4;
                }
            }
            h = Math.round(h * 30);
            if (h < 0) h += 180;
            
            const s = max === 0 ? 0 : Math.round((diff / max) * 255);
            const v = Math.round(max * 255);
            
            hsv[i] = h;
            hsv[i + 1] = s;
            hsv[i + 2] = v;
            hsv[i + 3] = data[i + 3];
        }
        
        return hsv;
    }

    detectColor(hsvData, ranges) {
        let pixelCount = 0;
        const threshold = 2000; // Seuil ajusté pour plus de précision
        
        for (let i = 0; i < hsvData.length; i += 4) {
            const h = hsvData[i];
            const s = hsvData[i + 1];
            const v = hsvData[i + 2];
            
            for (const range of ranges) {
                const [hMin, sMin, vMin, hMax, sMax, vMax] = range;
                
                if (s >= sMin && s <= sMax && v >= vMin && v <= vMax) {
                    if ((h >= hMin && h <= hMax) || 
                        (hMin > hMax && (h >= hMin || h <= hMax))) {
                        pixelCount++;
                        if (pixelCount >= threshold) {
                            return true;
                        }
                    }
                }
            }
        }
        
        return false;
    }

    async onColorDetected(colorName, config) {
        console.log(`Couleur détectée: ${colorName} (${config.label})`);
        
        // Mettre à jour la cadence
        this.updateCadence(colorName);
        
        // Capturer l'image actuelle
        const imageData = this.canvas.toDataURL('image/jpeg', 0.85);
        
        // Envoyer au backend
        try {
            const response = await fetch(`${this.API_BASE_URL}/api/detections`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify({
                    g_id: config.gId,
                    type_name: config.label,
                    color: colorName,
                    image_data: imageData
                })
            });

            if (response.ok) {
                const detection = await response.json();
                console.log('Détection sauvegardée:', detection.id);
                this.displayLatestCapture(detection);
                this.showDetectionAlert(detection);
                
                // Recharger les détections après un court délai
                setTimeout(() => {
                    this.loadDetections();
                }, 500);
            } else {
                console.error('Erreur sauvegarde détection:', response.status);
                this.updateStatus(`Erreur sauvegarde (${response.status})`, 'error');
            }
        } catch (error) {
            console.error('Erreur sauvegarde détection:', error);
            this.updateStatus('Erreur de connexion au serveur', 'error');
        }
    }

    showDetectionAlert(detection) {
        // Créer une notification visuelle
        const alert = document.createElement('div');
        alert.className = 'detection-alert';
        alert.style.cssText = `
            position: fixed;
            top: 20px;
            left: 20px;
            background: linear-gradient(45deg, #4CAF50, #45a049);
            color: white;
            padding: 15px 20px;
            border-radius: 8px;
            box-shadow: 0 4px 12px rgba(0,0,0,0.3);
            z-index: 10000;
            animation: slideIn 0.3s ease-out;
            font-weight: bold;
        `;
        
        alert.innerHTML = `
            <strong>Détection!</strong><br>
            ${detection.type_name} (${detection.color})<br>
            ID: ${detection.id}
        `;
        
        document.body.appendChild(alert);
        
        // Supprimer après 3 secondes
        setTimeout(() => {
            alert.style.animation = 'slideOut 0.3s ease-in';
            setTimeout(() => {
                if (alert.parentNode) {
                    alert.parentNode.removeChild(alert);
                }
            }, 300);
        }, 3000);
    }

    updateCadence(colorName) {
        const counter = this.cadenceCounters[colorName];
        counter.count++;
        
        const now = Date.now();
        const elapsedSeconds = (now - counter.startTime) / 1000;
        
        if (elapsedSeconds >= 1) {
            const cadence = counter.count / elapsedSeconds;
            
            // Mettre à jour l'affichage
            const cadenceElement = document.getElementById(`cadence-${colorName}`);
            if (cadenceElement) {
                cadenceElement.textContent = `${cadence.toFixed(1)} /s`;
                cadenceElement.style.color = cadence > 0.5 ? '#4CAF50' : '#666';
            }
            
            // Envoyer au backend
            this.sendCadenceUpdate(colorName, cadence);
        }
    }

    async sendCadenceUpdate(colorName, cadence) {
        try {
            const gId = this.colors[colorName].gId;
            await fetch(`${this.API_BASE_URL}/api/cadence`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify({
                    g_id: gId,
                    cadence: cadence
                })
            });
        } catch (error) {
            console.error('Erreur mise à jour cadence:', error);
        }
    }

    resetCadence() {
        console.log('Réinitialisation des cadences');
        
        this.cadenceCounters = {
            red: { count: 0, startTime: Date.now() },
            green: { count: 0, startTime: Date.now() },
            blue: { count: 0, startTime: Date.now() }
        };
        
        // Mettre à jour l'affichage
        ['red', 'green', 'blue'].forEach(color => {
            const element = document.getElementById(`cadence-${color}`);
            if (element) {
                element.textContent = '0.0 /s';
                element.style.color = '#666';
            }
        });
        
        this.updateStatus('Cadences réinitialisées', 'success');
    }

    updateStatus(message, type) {
        const statusElement = document.getElementById('detectionStatus');
        if (statusElement) {
            statusElement.textContent = message;
            statusElement.className = `detection-status ${type}`;
        }
    }

    updateButtonStates(cameraActive) {
        const startButton = document.getElementById('startCamera');
        const stopButton = document.getElementById('stopCamera');
        
        if (startButton) {
            startButton.disabled = cameraActive;
        }
        if (stopButton) {
            stopButton.disabled = !cameraActive;
        }
    }

    updateCadenceDisplay() {
        // Initialiser l'affichage des cadences
        ['red', 'green', 'blue'].forEach(color => {
            const element = document.getElementById(`cadence-${color}`);
            if (element) {
                element.textContent = '0.0 /s';
                element.style.color = '#666';
            }
        });
    }

    displayLatestCapture(detection) {
        const captureContainer = document.getElementById('latestCapture');
        if (captureContainer) {
            captureContainer.innerHTML = `
                <div class="capture-preview">
                    <img src="${detection.image_path}" alt="Capture ${detection.id}" style="max-width: 100%; border-radius: 5px;">
                    <div class="capture-info">
                        <strong>${detection.type_name}</strong>
                        <span>ID: ${detection.id}</span>
                        <span>${new Date(detection.date_time).toLocaleString()}</span>
                    </div>
                </div>
            `;
        }
    }

    async loadDetections() {
        try {
            const response = await fetch(`${this.API_BASE_URL}/api/history`);
            if (response.ok) {
                const data = await response.json();
                this.displayDetections(data.detections);
                
                // Mettre à jour le compteur total
                const totalElement = document.getElementById('totalDetections');
                if (totalElement) {
                    totalElement.textContent = data.detections.length;
                }
            }
        } catch (error) {
            console.error('Erreur chargement détections:', error);
        }
    }

    displayDetections(detections) {
        const tbody = document.getElementById('detectionsBody');
        if (!tbody) return;

        if (!detections || detections.length === 0) {
            tbody.innerHTML = `
                <tr>
                    <td colspan="5" style="text-align: center; padding: 20px; color: #666;">
                        Aucune détection pour le moment
                    </td>
                </tr>
            `;
            return;
        }

        // Prendre seulement les 10 dernières détections pour la vue de détection
        const recentDetections = detections.slice(0, 10);
        
        tbody.innerHTML = recentDetections.map(detection => {
            const date = new Date(detection.date_time);
            return `
                <tr>
                    <td>${detection.id}</td>
                    <td>${detection.type_name}</td>
                    <td>
                        <span class="color-indicator ${detection.color}"></span>
                        ${detection.color}
                    </td>
                    <td>${date.toLocaleString()}</td>
                    <td>
                        <img src="${detection.image_path}" alt="Capture" style="width: 50px; height: 50px; object-fit: cover; border-radius: 3px;">
                    </td>
                </tr>
            `;
        }).join('');
    }

    startHistoryAutoRefresh() {
        // Rafraîchir l'historique toutes les 30 secondes
        this.historyRefreshInterval = setInterval(() => {
            this.loadDetections();
        }, 30000);
    }

    cleanup() {
        console.log('Nettoyage des ressources...');
        
        this.stopCamera();
        
        if (this.historyRefreshInterval) {
            clearInterval(this.historyRefreshInterval);
        }
    }
}

// Initialiser le système au chargement de la page
document.addEventListener('DOMContentLoaded', () => {
    console.log('Initialisation du système de détection');
    
    // Vérifier que tous les éléments nécessaires sont présents
    const requiredElements = ['video', 'canvas'];
    const missingElements = requiredElements.filter(id => !document.getElementById(id));
    
    if (missingElements.length > 0) {
        console.error('Éléments manquants:', missingElements);
        return;
    }
    
    new ColorDetectionSystem();
});