// Configuration API
const API_BASE = 'http://127.0.0.1:3001/api';
let isAuthenticated = false;
let refreshInterval;
let allDetections = [];

// Initialisation au chargement de la page
document.addEventListener('DOMContentLoaded', function() {
    console.log('📊 Initialisation de la page historique...');
    checkAuthentication();
    setupDateInputs();
    
    // Démarrer l'actualisation automatique si connecté
    if (isAuthenticated) {
        startAutoRefresh();
    }
});

// Configuration des dates par défaut
function setupDateInputs() {
    const today = new Date().toISOString().split('T')[0];
    const oneWeekAgo = new Date(Date.now() - 7 * 24 * 60 * 60 * 1000).toISOString().split('T')[0];
    
    const startDateEl = document.getElementById('startDate');
    const endDateEl = document.getElementById('endDate');
    
    if (startDateEl) startDateEl.value = oneWeekAgo;
    if (endDateEl) endDateEl.value = today;
}

// Vérification de l'authentification
async function checkAuthentication() {
    console.log('🔐 Vérification de l\'authentification...');
    
    try {
        const token = localStorage.getItem('auth_token');
        if (!token) {
            console.log('❌ Aucun token trouvé');
            showLoginModal();
            return;
        }

        const response = await fetch(`${API_BASE}/verify`, {
            headers: {
                'Authorization': `Bearer ${token}`
            }
        });

        if (response.ok) {
            const data = await response.json();
            if (data.success) {
                console.log('✅ Authentification validée');
                isAuthenticated = true;
                showMainContainer();
                await loadDashboardData();
                startAutoRefresh();
                return;
            }
        }
    } catch (error) {
        console.error('❌ Erreur vérification auth:', error);
    }
    
    console.log('🔒 Authentification échouée');
    showLoginModal();
}

// Affichage du modal de login
function showLoginModal() {
    const loginModal = document.getElementById('loginModal');
    const mainContainer = document.getElementById('mainContainer');
    
    if (loginModal) loginModal.style.display = 'flex';
    if (mainContainer) mainContainer.style.display = 'none';
}

// Affichage du container principal
function showMainContainer() {
    const loginModal = document.getElementById('loginModal');
    const mainContainer = document.getElementById('mainContainer');
    
    if (loginModal) loginModal.style.display = 'none';
    if (mainContainer) mainContainer.style.display = 'block';
}

// Gestion du formulaire de login
document.getElementById('loginForm').addEventListener('submit', async function(e) {
    e.preventDefault();
    
    const username = document.getElementById('username').value;
    const password = document.getElementById('password').value;

    if (!username || !password) {
        showMessage('Veuillez remplir tous les champs', 'error');
        return;
    }

    try {
        const response = await fetch(`${API_BASE}/login`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify({ username, password })
        });

        const data = await response.json();
        
        if (data.success && data.token) {
            localStorage.setItem('auth_token', data.token);
            isAuthenticated = true;
            showMainContainer();
            await loadDashboardData();
            startAutoRefresh();
            showMessage('Connexion réussie!', 'success');
        } else {
            showMessage(data.message || 'Identifiants incorrects', 'error');
        }
    } catch (error) {
        console.error('❌ Erreur login:', error);
        showMessage('Erreur de connexion au serveur', 'error');
    }
});

// Chargement des données du dashboard
async function loadDashboardData() {
    console.log('📊 Chargement des données du dashboard...');
    
    try {
        await Promise.all([
            loadHistory(),
            loadTypes(),
            checkESP32Status()
        ]);
        console.log('✅ Toutes les données du dashboard chargées');
    } catch (error) {
        console.error('❌ Erreur chargement dashboard:', error);
    }
}

// Chargement de l'historique et calcul des statistiques
async function loadHistory() {
    console.log('📜 Chargement de l\'historique...');
    
    try {
        const token = localStorage.getItem('auth_token');
        if (!token) {
            throw new Error('Token manquant');
        }

        const response = await fetch(`${API_BASE}/history`, {
            headers: {
                'Authorization': `Bearer ${token}`
            }
        });

        if (response.ok) {
            const data = await response.json();
            console.log('📊 Données historique reçues:', data);
            
            // Extraire les détections selon la structure de l'API
            allDetections = data.detections || data || [];
            
            console.log('📊 Détections chargées:', allDetections.length);
            
            updateHistoryTable(allDetections);
            calculateAndDisplayStats();
            updateLastCapture(allDetections);
            
            return data;
        } else {
            throw new Error(`Erreur ${response.status}: ${response.statusText}`);
        }
    } catch (error) {
        console.error('❌ Erreur chargement historique:', error);
        
        // Afficher une erreur dans le tableau
        const tbody = document.getElementById('historyTableBody');
        if (tbody) {
            tbody.innerHTML = `
                <tr>
                    <td colspan="7" style="text-align: center; color: #e74c3c; padding: 2rem;">
                        Erreur lors du chargement: ${error.message}
                    </td>
                </tr>
            `;
        }
        
        // Réinitialiser les statistiques
        resetStats();
    }
}

// Calcul et affichage des statistiques
function calculateAndDisplayStats() {
    console.log('📈 Calcul des statistiques...');
    
    if (!allDetections || allDetections.length === 0) {
        console.log('📈 Aucune détection, statistiques à zéro');
        resetStats();
        return;
    }

    const today = new Date();
    const todayStr = today.toISOString().split('T')[0];
    
    console.log('📈 Date aujourd\'hui:', todayStr);
    console.log('📈 Nombre total de détections:', allDetections.length);

    // Filtrer les détections d'aujourd'hui
    const todayDetections = allDetections.filter(detection => {
        if (!detection.date_time) return false;
        
        const detectionDate = new Date(detection.date_time).toISOString().split('T')[0];
        return detectionDate === todayStr;
    });
    
    console.log('📈 Détections aujourd\'hui:', todayDetections.length);

    // Calculer les types uniques aujourd'hui
    const todayUniqueTypes = new Set();
    todayDetections.forEach(detection => {
        if (detection.g_id) {
            todayUniqueTypes.add(detection.g_id.toString());
        }
    });

    // Calculer les types uniques total
    const totalUniqueTypes = new Set();
    allDetections.forEach(detection => {
        if (detection.g_id) {
            totalUniqueTypes.add(detection.g_id.toString());
        }
    });

    console.log('📈 Types uniques aujourd\'hui:', todayUniqueTypes.size);
    console.log('📈 Types uniques total:', totalUniqueTypes.size);

    // Mettre à jour l'affichage des statistiques
    updateStatsDisplay(todayDetections.length, todayUniqueTypes.size, allDetections.length);
    
    // Afficher les statistiques par type
    displayTypeStats(todayDetections, allDetections);
}

// Mise à jour de l'affichage des statistiques
function updateStatsDisplay(todayCount, uniqueTypes, totalCount) {
    console.log('📊 Mise à jour affichage stats:', { todayCount, uniqueTypes, totalCount });
    
    const totalTodayEl = document.getElementById('totalToday');
    const uniqueTypesEl = document.getElementById('uniqueTypes');
    const totalHistoryEl = document.getElementById('totalHistory');
    
    if (totalTodayEl) {
        totalTodayEl.textContent = todayCount;
        console.log('✅ Total aujourd\'hui mis à jour:', todayCount);
    } else {
        console.warn('⚠️ Élément totalToday introuvable');
    }
    
    if (uniqueTypesEl) {
        uniqueTypesEl.textContent = uniqueTypes;
        console.log('✅ Types uniques mis à jour:', uniqueTypes);
    } else {
        console.warn('⚠️ Élément uniqueTypes introuvable');
    }
    
    if (totalHistoryEl) {
        totalHistoryEl.textContent = totalCount;
        console.log('✅ Total historique mis à jour:', totalCount);
    } else {
        console.warn('⚠️ Élément totalHistory introuvable');
    }
}

// Affichage des statistiques par type
function displayTypeStats(todayDetections, allDetections) {
    console.log('📊 Affichage des stats par type...');
    
    const typeStatsContainer = document.getElementById('typeStats');
    if (!typeStatsContainer) {
        console.warn('⚠️ Container typeStats introuvable');
        return;
    }

    // Compter les détections par type aujourd'hui
    const todayTypeCount = {};
    todayDetections.forEach(detection => {
        const key = `${detection.g_id || 'unknown'}`;
        if (!todayTypeCount[key]) {
            todayTypeCount[key] = {
                g_id: detection.g_id || 'unknown',
                type_name: detection.type_name || 'Type inconnu',
                color: detection.color || '#cccccc',
                count: 0
            };
        }
        todayTypeCount[key].count++;
    });

    // Compter le total par type dans l'historique complet
    const totalTypeCount = {};
    allDetections.forEach(detection => {
        const key = `${detection.g_id || 'unknown'}`;
        if (!totalTypeCount[key]) {
            totalTypeCount[key] = {
                g_id: detection.g_id || 'unknown',
                type_name: detection.type_name || 'Type inconnu',
                color: detection.color || '#cccccc',
                count: 0
            };
        }
        totalTypeCount[key].count++;
    });

    console.log('📊 Stats par type aujourd\'hui:', todayTypeCount);
    console.log('📊 Stats par type total:', totalTypeCount);

    // Afficher les statistiques
    if (Object.keys(todayTypeCount).length === 0) {
        typeStatsContainer.innerHTML = '<p style="color: #666; font-style: italic; text-align: center; padding: 1rem;">Aucune détection aujourd\'hui</p>';
        return;
    }

    let html = '';
    Object.values(todayTypeCount).forEach(type => {
        const totalForThisType = totalTypeCount[type.g_id]?.count || 0;
        
        html += `
            <div class="type-item">
                <div style="display: flex; align-items: center;">
                    <div class="type-color" style="background-color: ${type.color}"></div>
                    <span><strong>${type.type_name}</strong> (${type.g_id})</span>
                </div>
                <div style="text-align: right;">
                    <div style="font-weight: bold; color: #667eea;">Aujourd'hui: ${type.count}</div>
                    <small style="color: #666;">Total: ${totalForThisType}</small>
                </div>
            </div>
        `;
    });
    
    typeStatsContainer.innerHTML = html;
    console.log('✅ Stats par type affichées');
}

// Réinitialisation des statistiques
function resetStats() {
    console.log('🔄 Réinitialisation des statistiques');
    
    updateStatsDisplay(0, 0, 0);
    
    const typeStatsContainer = document.getElementById('typeStats');
    if (typeStatsContainer) {
        typeStatsContainer.innerHTML = '<p style="color: #666; font-style: italic; text-align: center; padding: 1rem;">Aucune donnée disponible</p>';
    }
}

// Mise à jour du tableau d'historique
function updateHistoryTable(detections) {
    console.log('📋 Mise à jour du tableau d\'historique...');
    
    const tbody = document.getElementById('historyTableBody');
    if (!tbody) {
        console.warn('⚠️ Tbody historique introuvable');
        return;
    }
    
    if (!detections || detections.length === 0) {
        tbody.innerHTML = `
            <tr>
                <td colspan="7" style="text-align: center; color: #666; padding: 2rem;">
                    Aucune détection dans l'historique
                </td>
            </tr>
        `;
        return;
    }

    // Trier par date décroissante (plus récent en premier)
    const sortedDetections = [...detections].sort((a, b) => {
        const dateA = new Date(a.date_time);
        const dateB = new Date(b.date_time);
        return dateB - dateA;
    });

    // Limiter à 100 entrées pour les performances
    const displayDetections = sortedDetections.slice(0, 100);
    
    console.log('📋 Affichage de', displayDetections.length, 'détections sur', sortedDetections.length);

    let html = '';
    displayDetections.forEach(detection => {
        const source = detection.image_path && detection.image_path.includes('esp32') ? 'ESP32' : 'Camera';
        const imageHtml = detection.image_path ? 
            `<img src="${detection.image_path}" alt="Capture" style="width: 40px; height: 30px; object-fit: cover; border-radius: 3px; cursor: pointer;" onclick="showImageModal('${detection.image_path}')">` 
            : 'N/A';
        
        html += `
            <tr>
                <td>${detection.id || 'N/A'}</td>
                <td>${detection.g_id || 'N/A'}</td>
                <td>${detection.type_name || 'N/A'}</td>
                <td>
                    <span class="color-dot" style="background-color: ${detection.color || '#ccc'}"></span>
                    ${detection.color || 'N/A'}
                </td>
                <td>${formatDateTime(detection.date_time)}</td>
                <td>${source}</td>
                <td>${imageHtml}</td>
            </tr>
        `;
    });
    
    tbody.innerHTML = html;
    console.log('✅ Tableau d\'historique mis à jour');
}

// Mise à jour de la dernière capture
function updateLastCapture(detections) {
    console.log('📸 Mise à jour de la dernière capture...');
    
    const capturePreview = document.getElementById('lastCapturePreview');
    const captureInfo = document.getElementById('lastCaptureInfo');
    
    if (!capturePreview || !captureInfo) {
        console.warn('⚠️ Éléments de capture introuvables');
        return;
    }
    
    if (!detections || detections.length === 0) {
        capturePreview.innerHTML = '<div class="no-capture">Aucune capture disponible</div>';
        captureInfo.innerHTML = '';
        return;
    }
    
    // Trouver la dernière détection avec image
    const sortedDetections = [...detections].sort((a, b) => {
        return new Date(b.date_time) - new Date(a.date_time);
    });
    
    const lastDetectionWithImage = sortedDetections.find(d => d.image_path);
    
    if (lastDetectionWithImage) {
        console.log('📸 Dernière capture trouvée:', lastDetectionWithImage);
        
        capturePreview.innerHTML = `
            <img src="${lastDetectionWithImage.image_path}" 
                 alt="Dernière capture" 
                 onclick="showImageModal('${lastDetectionWithImage.image_path}')"
                 style="cursor: pointer;">
        `;
        
        captureInfo.innerHTML = `
            <div style="font-size: 0.9rem; color: #666;">
                <strong>Type:</strong> ${lastDetectionWithImage.type_name || 'N/A'} (ID: ${lastDetectionWithImage.g_id || 'N/A'})<br>
                <strong>Couleur:</strong> 
                <span class="color-dot" style="background-color: ${lastDetectionWithImage.color || '#ccc'}; margin-right: 5px;"></span>
                ${lastDetectionWithImage.color || 'N/A'}<br>
                <strong>Date:</strong> ${formatDateTime(lastDetectionWithImage.date_time)}<br>
                <strong>Source:</strong> ${lastDetectionWithImage.image_path && lastDetectionWithImage.image_path.includes('esp32') ? 'ESP32' : 'Camera'}
            </div>
        `;
    } else {
        console.log('📸 Aucune capture avec image trouvée');
        capturePreview.innerHTML = '<div class="no-capture">Aucune capture avec image disponible</div>';
        captureInfo.innerHTML = '';
    }
}

// Formatage de la date/heure
function formatDateTime(dateTime) {
    if (!dateTime) return 'N/A';
    
    try {
        const date = new Date(dateTime);
        return date.toLocaleString('fr-FR', {
            day: '2-digit',
            month: '2-digit', 
            year: 'numeric',
            hour: '2-digit',
            minute: '2-digit',
            second: '2-digit'
        });
    } catch (error) {
        console.warn('⚠️ Erreur formatage date:', error);
        return dateTime.toString();
    }
}

// Vérification du statut ESP32
async function checkESP32Status() {
    console.log('📡 Vérification du statut ESP32...');
    
    const statusElement = document.getElementById('esp32Status');
    const statusIndicator = document.getElementById('statusIndicator');
    
    try {
        const token = localStorage.getItem('auth_token');
        const response = await fetch(`${API_BASE}/history`, {
            headers: {
                'Authorization': `Bearer ${token}`
            }
        });

        if (response.ok) {
            const data = await response.json();
            const detections = data.detections || data || [];
            
            if (detections.length > 0) {
                // Vérifier si il y a des détections récentes (dernières 5 minutes)
                const fiveMinutesAgo = new Date(Date.now() - 5 * 60 * 1000);
                const recentDetections = detections.filter(d => 
                    new Date(d.date_time) > fiveMinutesAgo
                );
                
                if (recentDetections.length > 0) {
                    updateESP32Status(true, `Actif (${recentDetections.length} détections récentes)`);
                } else {
                    updateESP32Status(true, 'Connecté (pas d\'activité récente)');
                }
            } else {
                updateESP32Status(true, 'Connecté (aucune détection)');
            }
        } else {
            updateESP32Status(false, 'Erreur de connexion');
        }
    } catch (error) {
        console.error('❌ Erreur statut ESP32:', error);
        updateESP32Status(false, 'Non accessible');
    }
}

// Mise à jour du statut ESP32
function updateESP32Status(isConnected, statusText) {
    const statusElement = document.getElementById('esp32Status');
    const statusIndicator = document.getElementById('statusIndicator');
    
    if (statusElement) {
        statusElement.textContent = statusText;
        statusElement.className = isConnected ? 'esp32-status connected' : 'esp32-status disconnected';
    }
    
    if (statusIndicator) {
        statusIndicator.style.background = isConnected ? '#4CAF50' : '#f44336';
    }
    
    console.log(`📡 Statut ESP32: ${isConnected ? '✅' : '❌'} ${statusText}`);
}

// Affichage modal pour les images
function showImageModal(imagePath) {
    console.log('🖼️ Affichage modal pour:', imagePath);
    
    const modal = document.createElement('div');
    modal.style.cssText = `
        position: fixed;
        top: 0;
        left: 0;
        width: 100%;
        height: 100%;
        background: rgba(0, 0, 0, 0.9);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 10000;
        cursor: pointer;
        animation: fadeIn 0.3s ease;
    `;
    
    modal.innerHTML = `
        <div style="position: relative; max-width: 90%; max-height: 90%;">
            <img src="${imagePath}" 
                 alt="Capture agrandie" 
                 style="max-width: 100%; max-height: 100%; border-radius: 10px; 
                        box-shadow: 0 10px 30px rgba(0,0,0,0.5);">
            <button onclick="this.parentElement.parentElement.remove()" 
                    style="position: absolute; top: -10px; right: -10px; 
                           background: white; border: none; width: 30px; height: 30px; 
                           border-radius: 50%; cursor: pointer; font-size: 16px;
                           box-shadow: 0 2px 10px rgba(0,0,0,0.3);">×</button>
        </div>
    `;
    
    // Fermer en cliquant sur l'arrière-plan
    modal.onclick = (e) => {
        if (e.target === modal) {
            modal.remove();
        }
    };
    
    // Fermer avec Échap
    const closeOnEscape = (e) => {
        if (e.key === 'Escape') {
            modal.remove();
            document.removeEventListener('keydown', closeOnEscape);
        }
    };
    document.addEventListener('keydown', closeOnEscape);
    
    document.body.appendChild(modal);
}

// Export des données
async function exportData(format) {
    console.log(`📄 Export en format ${format}...`);
    
    const startDate = document.getElementById('startDate')?.value;
    const endDate = document.getElementById('endDate')?.value;

    if (!startDate || !endDate) {
        showMessage('Veuillez sélectionner les dates de début et de fin', 'error');
        return;
    }

    if (new Date(startDate) > new Date(endDate)) {
        showMessage('La date de début doit être antérieure à la date de fin', 'error');
        return;
    }

    try {
        showMessage(`Export ${format.toUpperCase()} en cours...`, 'info');
        
        const token = localStorage.getItem('auth_token');
        const response = await fetch(`${API_BASE}/history/export`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${token}`
            },
            body: JSON.stringify({
                start_date: startDate,
                end_date: endDate,
                format: format
            })
        });

        if (response.ok) {
            const data = await response.json();
            if (data.success) {
                // Créer et télécharger le fichier
                const blob = new Blob([data.content], { 
                    type: format === 'csv' ? 'text/csv' : 'text/plain' 
                });
                const url = window.URL.createObjectURL(blob);
                const a = document.createElement('a');
                a.href = url;
                a.download = data.filename;
                document.body.appendChild(a);
                a.click();
                document.body.removeChild(a);
                window.URL.revokeObjectURL(url);
                
                showMessage(`Export ${format.toUpperCase()} téléchargé avec succès`, 'success');
                console.log(`✅ Export ${format} réussi: ${data.filename}`);
            } else {
                throw new Error(data.message || 'Erreur inconnue');
            }
        } else {
            const errorData = await response.json().catch(() => ({}));
            throw new Error(errorData.message || `Erreur ${response.status}`);
        }
    } catch (error) {
        console.error('❌ Erreur export:', error);
        showMessage(`Erreur lors de l'export: ${error.message}`, 'error');
    }
}

// Chargement des types existants
async function loadTypes() {
    console.log('🏷️ Chargement des types...');
    
    try {
        const token = localStorage.getItem('auth_token');
        const response = await fetch(`${API_BASE}/types`, {
            headers: {
                'Authorization': `Bearer ${token}`
            }
        });

        if (response.ok) {
            const types = await response.json();
            console.log('🏷️ Types chargés:', types);
            updateTypesDisplay(types);
        } else {
            throw new Error(`Erreur ${response.status}`);
        }
    } catch (error) {
        console.error('❌ Erreur chargement types:', error);
        const container = document.getElementById('existingTypes');
        if (container) {
            container.innerHTML = '<p style="color: #e74c3c; font-style: italic;">Erreur de chargement des types</p>';
        }
    }
}

// Mise à jour de l'affichage des types
function updateTypesDisplay(types) {
    const container = document.getElementById('existingTypes');
    if (!container) {
        console.warn('⚠️ Container existingTypes introuvable');
        return;
    }
    
    if (!types || types.length === 0) {
        container.innerHTML = '<p style="color: #666; font-style: italic; text-align: center; padding: 1rem;">Aucun type configuré</p>';
        return;
    }
    
    let html = '';
    types.forEach(type => {
        html += `
            <div class="type-item" style="font-size: 0.9rem;">
                <div style="display: flex; align-items: center;">
                    <div class="type-color" style="background-color: ${type.color || '#ccc'}; margin-right: 8px;"></div>
                    <span><strong>${type.g_id}</strong> - ${type.type_name}</span>
                </div>
                <button onclick="editType('${type.g_id}', '${type.type_name}', '${type.color || '#667eea'}')" 
                        style="background: none; border: 1px solid #667eea; color: #667eea; 
                               padding: 0.2rem 0.5rem; border-radius: 3px; cursor: pointer;
                               font-size: 0.8rem;">
                    Modifier
                </button>
            </div>
        `;
    });
    
    container.innerHTML = html;
    console.log('✅ Types affichés:', types.length);
}

// Ajout ou modification d'un type
async function addOrUpdateType() {
    console.log('🏷️ Ajout/modification d\'un type...');
    
    const gId = document.getElementById('newGId')?.value;
    const typeName = document.getElementById('newTypeName')?.value;
    const color = document.getElementById('newColor')?.value;

    if (!gId || !typeName) {
        showMessage('Veuillez remplir le G_ID et le nom du type', 'error');
        return;
    }

    // Validation du G_ID (doit être numérique)
    if (isNaN(gId)) {
        showMessage('Le G_ID doit être numérique', 'error');
        return;
    }

    try {
        showMessage('Ajout du type en cours...', 'info');
        
        const token = localStorage.getItem('auth_token');
        const response = await fetch(`${API_BASE}/types`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${token}`
            },
            body: JSON.stringify({
                g_id: parseInt(gId),
                type_name: typeName.trim(),
                color: color
            })
        });

        const data = await response.json();
        
        if (response.ok && data.success) {
            showMessage('Type ajouté/modifié avec succès', 'success');
            console.log('✅ Type ajouté:', data);
            
            // Réinitialiser le formulaire
            document.getElementById('newGId').value = '';
            document.getElementById('newTypeName').value = '';
            document.getElementById('newColor').value = '#667eea';
            
            // Recharger les types
            await loadTypes();
        } else {
            throw new Error(data.message || 'Erreur inconnue');
        }
    } catch (error) {
        console.error('❌ Erreur ajout type:', error);
        showMessage(`Erreur lors de l'ajout du type: ${error.message}`, 'error');
    }
}

// Modification d'un type existant
function editType(gId, typeName, color) {
    console.log('✏️ Édition du type:', { gId, typeName, color });
    
    const gIdInput = document.getElementById('newGId');
    const typeNameInput = document.getElementById('newTypeName');
    const colorInput = document.getElementById('newColor');
    
    if (gIdInput) gIdInput.value = gId;
    if (typeNameInput) typeNameInput.value = typeName;
    if (colorInput) colorInput.value = color;
    
    // Scroll vers le formulaire et focus
    const form = document.querySelector('.type-form');
    if (form) {
        form.scrollIntoView({ behavior: 'smooth', block: 'center' });
        setTimeout(() => {
            if (typeNameInput) typeNameInput.focus();
        }, 500);
    }
}

// Suppression de l'historique
async function clearHistory() {
    console.log('🗑️ Demande de suppression de l\'historique...');
    
    const confirmed = confirm('⚠️ ATTENTION !\n\nÊtes-vous sûr de vouloir supprimer TOUT l\'historique ?\n\nCette action est IRRÉVERSIBLE et supprimera :\n- Toutes les détections\n- Toutes les images\n- Toutes les statistiques\n\nTaper "SUPPRIMER" pour confirmer');
    
    if (!confirmed) {
        console.log('❌ Suppression annulée par l\'utilisateur');
        return;
    }

    const doubleConfirm = prompt('Pour confirmer la suppression complète de l\'historique, tapez exactement "SUPPRIMER" (en majuscules) :');
    
    if (doubleConfirm !== 'SUPPRIMER') {
        console.log('❌ Confirmation incorrecte, suppression annulée');
        showMessage('Suppression annulée - confirmation incorrecte', 'info');
        return;
    }

    try {
        showMessage('Suppression de l\'historique en cours...', 'info');
        console.log('🗑️ Exécution de la suppression...');
        
        const token = localStorage.getItem('auth_token');
        const response = await fetch(`${API_BASE}/history?confirm=true`, {
            method: 'DELETE',
            headers: {
                'Authorization': `Bearer ${token}`
            }
        });

        if (response.ok) {
            const data = await response.json();
            if (data.success) {
                showMessage('Historique supprimé avec succès', 'success');
                console.log('✅ Historique supprimé avec succès');
                
                // Réinitialiser les données locales
                allDetections = [];
                updateHistoryTable([]);
                resetStats();
                updateLastCapture([]);
                
                // Mettre à jour le statut ESP32
                setTimeout(() => {
                    checkESP32Status();
                }, 1000);
            } else {
                throw new Error(data.message || 'Erreur inconnue');
            }
        } else {
            const errorData = await response.json().catch(() => ({}));
            throw new Error(errorData.message || `Erreur ${response.status}`);
        }
    } catch (error) {
        console.error('❌ Erreur suppression historique:', error);
        showMessage(`Erreur lors de la suppression: ${error.message}`, 'error');
    }
}

// Actualisation automatique
function startAutoRefresh() {
    console.log('🔄 Démarrage de l\'actualisation automatique (30s)...');
    
    // Nettoyer l'ancien intervalle s'il existe
    if (refreshInterval) {
        clearInterval(refreshInterval);
    }
    
    refreshInterval = setInterval(async () => {
        try {
            console.log('🔄 Actualisation automatique...');
            
            // Vérifier d'abord l'authentification
            if (!isAuthenticated) {
                console.log('🔒 Plus authentifié, arrêt de l\'actualisation');
                stopAutoRefresh();
                return;
            }
            
            // Actualiser les données silencieusement
            await loadHistory();
            await checkESP32Status();
            
            console.log('✅ Actualisation automatique réussie');
        } catch (error) {
            console.error('❌ Erreur lors de l\'actualisation automatique:', error);
            
            // Si erreur d'authentification, arrêter l'actualisation
            if (error.message && error.message.includes('401')) {
                console.log('🔒 Erreur d\'authentification, arrêt de l\'actualisation');
                stopAutoRefresh();
                showMessage('Session expirée, reconnexion nécessaire', 'error');
            }
        }
    }, 30000); // 30 secondes
}

function stopAutoRefresh() {
    if (refreshInterval) {
        console.log('⏹️ Arrêt de l\'actualisation automatique');
        clearInterval(refreshInterval);
        refreshInterval = null;
    }
}

// Affichage des messages
function showMessage(message, type = 'success') {
    console.log(`💬 Message ${type}:`, message);
    
    // Supprimer l'ancien message s'il existe
    const existingMessage = document.getElementById('messageDiv');
    if (existingMessage) {
        existingMessage.remove();
    }
    
    // Créer le nouveau message
    const messageDiv = document.createElement('div');
    messageDiv.id = 'messageDiv';
    messageDiv.className = `message ${type}`;
    messageDiv.textContent = message;
    messageDiv.style.display = 'block';
    
    // Styles selon le type
    let backgroundColor, textColor, borderColor;
    switch (type) {
        case 'success':
            backgroundColor = '#d4edda';
            textColor = '#155724';
            borderColor = '#c3e6cb';
            break;
        case 'error':
            backgroundColor = '#f8d7da';
            textColor = '#721c24';
            borderColor = '#f5c6cb';
            break;
        case 'info':
            backgroundColor = '#d1ecf1';
            textColor = '#0c5460';
            borderColor = '#bee5eb';
            break;
        default:
            backgroundColor = '#e2e3e5';
            textColor = '#383d41';
            borderColor = '#d6d8db';
    }
    
    messageDiv.style.cssText = `
        position: fixed;
        top: 20px;
        right: 20px;
        padding: 15px 20px;
        background: ${backgroundColor};
        color: ${textColor};
        border: 1px solid ${borderColor};
        border-radius: 8px;
        box-shadow: 0 4px 12px rgba(0,0,0,0.1);
        z-index: 9999;
        max-width: 400px;
        font-size: 14px;
        line-height: 1.4;
        animation: slideIn 0.3s ease-out;
    `;
    
    // Ajouter l'animation CSS si elle n'existe pas
    if (!document.getElementById('messageStyles')) {
        const style = document.createElement('style');
        style.id = 'messageStyles';
        style.textContent = `
            @keyframes slideIn {
                from {
                    transform: translateX(100%);
                    opacity: 0;
                }
                to {
                    transform: translateX(0);
                    opacity: 1;
                }
            }
            @keyframes slideOut {
                from {
                    transform: translateX(0);
                    opacity: 1;
                }
                to {
                    transform: translateX(100%);
                    opacity: 0;
                }
            }
        `;
        document.head.appendChild(style);
    }
    
    document.body.appendChild(messageDiv);

    // Auto-suppression après 5 secondes
    setTimeout(() => {
        if (messageDiv.parentNode) {
            messageDiv.style.animation = 'slideOut 0.3s ease-out';
            setTimeout(() => {
                if (messageDiv.parentNode) {
                    messageDiv.parentNode.removeChild(messageDiv);
                }
            }, 300);
        }
    }, 5000);
}

// Gestion des événements de visibilité de la page
document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'visible') {
        console.log('👁️ Page visible - vérification du statut');
        if (isAuthenticated) {
            // Actualiser les données quand la page redevient visible
            setTimeout(() => {
                loadHistory();
                checkESP32Status();
            }, 1000);
        }
    } else {
        console.log('👁️ Page masquée');
    }
});

// Nettoyage lors de la fermeture de la page
window.addEventListener('beforeunload', () => {
    console.log('🚪 Fermeture de la page - nettoyage');
    stopAutoRefresh();
});

// Gestion des erreurs globales
window.addEventListener('error', (event) => {
    console.error('❌ Erreur globale:', event.error);
});

window.addEventListener('unhandledrejection', (event) => {
    console.error('❌ Promise rejetée:', event.reason);
    // Éviter l'affichage d'erreurs pour les timeouts réseau
    if (event.reason && event.reason.name !== 'AbortError') {
        showMessage('Erreur de connexion réseau', 'error');
    }
});

// Raccourcis clavier utiles
document.addEventListener('keydown', (e) => {
    // F5 ou Ctrl+R pour actualiser
    if (e.key === 'F5' || (e.ctrlKey && e.key === 'r')) {
        e.preventDefault();
        console.log('⌨️ Actualisation forcée via clavier');
        loadDashboardData();
    }
    
    // Ctrl+E pour export CSV rapide
    if (e.ctrlKey && e.key === 'e') {
        e.preventDefault();
        console.log('⌨️ Export CSV via clavier');
        exportData('csv');
    }
    
    // Ctrl+D pour supprimer l'historique (avec confirmation)
    if (e.ctrlKey && e.shiftKey && e.key === 'D') {
        e.preventDefault();
        console.log('⌨️ Suppression historique via clavier');
        clearHistory();
    }
});

// Interface publique pour compatibilité
window.HistoryManager = {
    loadHistory,
    exportData,
    clearHistory,
    addOrUpdateType,
    checkESP32Status,
    showMessage
};

console.log('✅ history.js entièrement chargé et initialisé');