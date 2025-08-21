class ColorDetectionSystem {
    constructor() {
        this.video = document.getElementById('video');
        this.canvas = document.getElementById('canvas');
        this.ctx = this.canvas.getContext('2d');
        this.stream = null;
        this.isDetecting = false;
        this.detectionInterval = null;
        
        // Compteurs de cadence
        this.cadenceCounters = {
            red: { count: 0, startTime: Date.now() },
            green: { count: 0, startTime: Date.now() },
            blue: { count: 0, startTime: Date.now() }
        };
        
        // Configuration des couleurs HSV
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
        
        this.initEventListeners();
        this.loadDetections();
        this.updateCadenceDisplay();
    }

    initEventListeners() {
        document.getElementById('startCamera').addEventListener('click', () => this.startCamera());
        document.getElementById('stopCamera').addEventListener('click', () => this.stopCamera());
        document.getElementById('resetCadence').addEventListener('click', () => this.resetCadence());
        document.getElementById('refreshDB').addEventListener('click', () => this.loadDetections());
    }

    async startCamera() {
        try {
            this.stream = await navigator.mediaDevices.getUserMedia({ 
                video: { width: 640, height: 480 } 
            });
            this.video.srcObject = this.stream;
            
            this.video.onloadedmetadata = () => {
                this.canvas.width = this.video.videoWidth;
                this.canvas.height = this.video.videoHeight;
                this.startDetection();
            };
            
            this.updateStatus('Caméra démarrée - Détection active');
        } catch (error) {
            console.error('Erreur accès caméra:', error);
            this.updateStatus('Erreur: Impossible d\'accéder à la caméra');
        }
    }

    stopCamera() {
        if (this.stream) {
            this.stream.getTracks().forEach(track => track.stop());
            this.stream = null;
        }
        this.stopDetection();
        this.updateStatus('Caméra arrêtée');
    }

    startDetection() {
        this.isDetecting = true;
        this.detectionInterval = setInterval(() => {
            this.processFrame();
        }, 100); // Détection toutes les 100ms
    }

    stopDetection() {
        this.isDetecting = false;
        if (this.detectionInterval) {
            clearInterval(this.detectionInterval);
            this.detectionInterval = null;
        }
    }

    processFrame() {
        if (!this.video.videoWidth || !this.video.videoHeight) return;

        // Capturer le frame
        this.ctx.drawImage(this.video, 0, 0, this.canvas.width, this.canvas.height);
        const imageData = this.ctx.getImageData(0, 0, this.canvas.width, this.canvas.height);
        const hsv = this.rgbToHsv(imageData);

        // Détecter chaque couleur
        for (const [colorName, config] of Object.entries(this.colors)) {
            if (this.detectColor(hsv, config.ranges)) {
                this.onColorDetected(colorName, config);
            }
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
        const threshold = 1000; // Nombre minimum de pixels pour détecter
        
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
        // Mettre à jour la cadence
        this.updateCadence(colorName);
        
        // Capturer l'image
        const imageData = this.canvas.toDataURL('image/jpeg', 0.8);
        
        // Envoyer au backend
        try {
            const response = await fetch('/api/detections', {
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
                this.displayLatestCapture(detection);
                this.loadDetections(); // Actualiser la table
            }
        } catch (error) {
            console.error('Erreur sauvegarde détection:', error);
        }
    }

    updateCadence(colorName) {
        const counter = this.cadenceCounters[colorName];
        counter.count++;
        
        const now = Date.now();
        const elapsedSeconds = (now - counter.startTime) / 1000;
        
        if (elapsedSeconds >= 1) {
            const cadence = counter.count / elapsedSeconds;
            
            // Mettre à jour l'affichage
            document.getElementById(`cadence-${colorName}`).textContent = `${cadence.toFixed(1)} /s`;
            
            // Envoyer au backend
            this.sendCadenceUpdate(this.colors[colorName].gId, cadence);
            
            // Réinitialiser le compteur
            counter.count = 0;
            counter.startTime = now;
        }
    }

    async sendCadenceUpdate(gId, cadence) {
        try {
            await fetch('/api/cadence', {
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
        for (const colorName in this.cadenceCounters) {
            this.cadenceCounters[colorName] = { count: 0, startTime: Date.now() };
            document.getElementById(`cadence-${colorName}`).textContent = '0.0 /s';
        }
    }

    updateCadenceDisplay() {
        setInterval(() => {
            for (const colorName in this.cadenceCounters) {
                const counter = this.cadenceCounters[colorName];
                const elapsedSeconds = (Date.now() - counter.startTime) / 1000;
                
                if (elapsedSeconds >= 5 && counter.count === 0) {
                    document.getElementById(`cadence-${colorName}`).textContent = '0.0 /s';
                }
            }
        }, 1000);
    }

    displayLatestCapture(detection) {
        const captureDiv = document.getElementById('latestCapture');
        captureDiv.innerHTML = `
            <div class="capture-preview">
                <img src="${detection.image_path}" alt="Capture ${detection.id}">
                <div class="capture-info">
                    <strong>ID:</strong> ${detection.id}<br>
                    <strong>Type:</strong> ${detection.type_name}<br>
                    <strong>Couleur:</strong> ${detection.color}<br>
                    <strong>Heure:</strong> ${new Date(detection.date_time).toLocaleString()}
                </div>
            </div>
        `;
    }

    async loadDetections() {
        try {
            const response = await fetch('/api/history');
            if (response.ok) {
                const data = await response.json();
                this.displayDetections(data.detections.slice(0, 20)); // 20 dernières détections
                document.getElementById('totalDetections').textContent = data.detections.length;
            }
        } catch (error) {
            console.error('Erreur chargement détections:', error);
        }
    }

    displayDetections(detections) {
        const tbody = document.getElementById('detectionsBody');
        tbody.innerHTML = '';

        detections.forEach(detection => {
            const row = document.createElement('tr');
            row.innerHTML = `
                <td>${detection.id}</td>
                <td>${detection.type_name}</td>
                <td>
                    <span class="color-indicator ${detection.color}"></span>
                    ${detection.color}
                </td>
                <td>${new Date(detection.date_time).toLocaleString()}</td>
                <td>
                    <img src="${detection.image_path}" 
                         alt="Capture ${detection.id}" 
                         class="capture-thumb"
                         onclick="this.style.transform = this.style.transform ? '' : 'scale(3)'; this.style.zIndex = this.style.zIndex ? '' : '1000'; this.style.position = this.style.position ? '' : 'relative';">
                </td>
            `;
            tbody.appendChild(row);
        });
    }

    updateStatus(message) {
        document.getElementById('detectionStatus').textContent = message;
    }
}

// Initialiser le système au chargement de la page
document.addEventListener('DOMContentLoaded', () => {
    new ColorDetectionSystem();
});