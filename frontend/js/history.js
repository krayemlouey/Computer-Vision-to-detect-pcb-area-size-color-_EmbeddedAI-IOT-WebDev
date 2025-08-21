class HistoryManager {
    constructor() {
        this.historyData = null;
        this.initEventListeners();
        this.checkAuthentication();
        this.loadHistory();
        this.setDefaultDates();
    }

    checkAuthentication() {
        if (!window.AuthManager.isAuthenticated()) {
            window.location.href = 'login.html';
            return;
        }
    }

    initEventListeners() {
        // Boutons principaux
        document.getElementById('logoutBtn').addEventListener('click', () => {
            window.AuthManager.logout();
        });
        
        document.getElementById('refreshHistory').addEventListener('click', () => {
            this.loadHistory();
        });
        
        document.getElementById('deleteHistory').addEventListener('click', () => {
            this.showDeleteConfirmation();
        });

        // Export
        document.getElementById('exportCSV').addEventListener('click', () => {
            this.exportHistory('csv');
        });
        
        document.getElementById('exportTXT').addEventListener('click', () => {
            this.exportHistory('txt');
        });

        // Ajout de type
        document.getElementById('addTypeForm').addEventListener('submit', (e) => {
            this.handleAddType(e);
        });

        // Modal de confirmation
        document.getElementById('confirmYes').addEventListener('click', () => {
            this.executeConfirmedAction();
        });
        
        document.getElementById('confirmNo').addEventListener('click', () => {
            this.hideModal();
        });
    }

    setDefaultDates() {
        const today = new Date();
        const lastWeek = new Date(today);
        lastWeek.setDate(today.getDate() - 7);
        
        document.getElementById('endDate').value = today.toISOString().split('T')[0];
        document.getElementById('startDate').value = lastWeek.toISOString().split('T')[0];
    }

    async loadHistory() {
        this.showLoading(true);
        
        try {
            const response = await window.AuthManager.makeAuthenticatedRequest('/api/history');
            
            if (response.ok) {
                this.historyData = await response.json();
                this.displayHistory();
                this.updateStats();
            } else {
                throw new Error('Erreur lors du chargement');
            }
        } catch (error) {
            console.error('Erreur chargement historique:', error);
            this.showError('Erreur lors du chargement de l\'historique');
        } finally {
            this.showLoading(false);
        }
    }

    displayHistory() {
        const container = document.getElementById('historyContent');
        container.innerHTML = '';

        if (!this.historyData.detections.length) {
            document.getElementById('noHistory').style.display = 'block';
            return;
        }

        document.getElementById('noHistory').style.display = 'none';

        // Trier les dates en ordre décroissant
        const sortedDates = Object.keys(this.historyData.grouped_by_date)
            .sort((a, b) => new Date(b) - new Date(a));

        sortedDates.forEach(date => {
            const detections = this.historyData.grouped_by_date[date];
            const dateGroup = this.createDateGroup(date, detections);
            container.appendChild(dateGroup);
        });
    }

    createDateGroup(date, detections) {
        const groupDiv = document.createElement('div');
        groupDiv.className = 'date-group';

        const dateHeader = document.createElement('div');
        dateHeader.className = 'date-header';
        dateHeader.innerHTML = `
            <span class="date-title">${this.formatDate(date)}</span>
            <span class="date-count">${detections.length}</span>
        `;

        const detectionGrid = document.createElement('div');
        detectionGrid.className = 'detection-grid';

        detections.forEach(detection => {
            const card = this.createDetectionCard(detection);
            detectionGrid.appendChild(card);
        });

        groupDiv.appendChild(dateHeader);
        groupDiv.appendChild(detectionGrid);

        return groupDiv;
    }

    createDetectionCard(detection) {
        const card = document.createElement('div');
        card.className = `detection-card ${detection.color}`;
        
        const time = new Date(detection.date_time).toLocaleTimeString();
        
        card.innerHTML = `
            <div class="detection-header">
                <span class="detection-id">${detection.id}</span>
                <span class="detection-time">${time}</span>
            </div>
            <div class="detection-type">${detection.type_name}</div>
            <img src="${detection.image_path}" 
                 alt="Capture ${detection.id}" 
                 class="detection-image"
                 onclick="this.style.position='fixed'; this.style.top='50%'; this.style.left='50%'; this.style.transform='translate(-50%, -50%) scale(2)'; this.style.zIndex='10000'; this.style.background='rgba(0,0,0,0.8)'; this.style.padding='20px'; this.style.borderRadius='10px'; this.onclick=function(){this.style.cssText='';}">
        `;

        return card;
    }

    updateStats() {
        if (!this.historyData) return;

        const total = this.historyData.detections.length;
        const today = new Date().toISOString().split('T')[0];
        const thisWeek = new Date();
        thisWeek.setDate(thisWeek.getDate() - 7);

        const todayCount = this.historyData.detections.filter(d => 
            d.date_time.startsWith(today)
        ).length;

        const weekCount = this.historyData.detections.filter(d => 
            new Date(d.date_time) >= thisWeek
        ).length;

        document.getElementById('totalCount').textContent = total;
        document.getElementById('todayCount').textContent = todayCount;
        document.getElementById('weekCount').textContent = weekCount;
    }

    async exportHistory(format) {
        const startDate = document.getElementById('startDate').value;
        const endDate = document.getElementById('endDate').value;

        if (!startDate || !endDate) {
            alert('Veuillez sélectionner une période');
            return;
        }

        if (new Date(startDate) > new Date(endDate)) {
            alert('La date de début doit être antérieure à la date de fin');
            return;
        }

        try {
            const response = await window.AuthManager.makeAuthenticatedRequest('/api/history/export', {
                method: 'POST',
                body: JSON.stringify({
                    start_date: startDate,
                    end_date: endDate,
                    format: format
                })
            });

            if (response.ok) {
                const content = await response.text();
                this.downloadFile(content, `historique_${startDate}_${endDate}.${format}`);
            } else {
                throw new Error('Erreur lors de l\'export');
            }
        } catch (error) {
            console.error('Erreur export:', error);
            alert('Erreur lors de l\'export des données');
        }
    }

    downloadFile(content, filename) {
        const blob = new Blob([content], { type: 'text/plain;charset=utf-8' });
        const url = window.URL.createObjectURL(blob);
        const link = document.createElement('a');
        link.href = url;
        link.download = filename;
        document.body.appendChild(link);
        link.click();
        document.body.removeChild(link);
        window.URL.revokeObjectURL(url);
    }

    async handleAddType(event) {
        event.preventDefault();
        
        const gId = document.getElementById('newGId').value.trim();
        const typeName = document.getElementById('newType').value.trim();
        const color = document.getElementById('newColor').value;

        if (!gId || !typeName || !color) {
            alert('Veuillez remplir tous les champs');
            return;
        }

        if (!/^\d{4}$/.test(gId)) {
            alert('Le G_ID doit contenir exactement 4 chiffres');
            return;
        }

        try {
            const response = await window.AuthManager.makeAuthenticatedRequest('/api/types', {
                method: 'POST',
                body: JSON.stringify({
                    g_id: gId,
                    type_name: typeName,
                    color: color
                })
            });

            if (response.ok) {
                alert('Nouveau type ajouté avec succès !');
                document.getElementById('addTypeForm').reset();
            } else {
                const error = await response.text();
                throw new Error(error);
            }
        } catch (error) {
            console.error('Erreur ajout type:', error);
            if (error.message.includes('UNIQUE constraint')) {
                alert('Ce G_ID existe déjà. Veuillez utiliser un identifiant unique.');
            } else {
                alert('Erreur lors de l\'ajout du nouveau type');
            }
        }
    }

    showDeleteConfirmation() {
        this.pendingAction = 'delete';
        document.getElementById('confirmMessage').textContent = 
            'Êtes-vous sûr de vouloir supprimer tout l\'historique ? Cette action est irréversible.';
        this.showModal();
    }

    async executeConfirmedAction() {
        if (this.pendingAction === 'delete') {
            await this.deleteHistory();
        }
        this.hideModal();
    }

    async deleteHistory() {
        try {
            const response = await window.AuthManager.makeAuthenticatedRequest('/api/history', {
                method: 'DELETE'
            });

            if (response.ok) {
                alert('Historique supprimé avec succès');
                this.loadHistory(); // Recharger pour afficher la page vide
            } else {
                throw new Error('Erreur lors de la suppression');
            }
        } catch (error) {
            console.error('Erreur suppression:', error);
            alert('Erreur lors de la suppression de l\'historique');
        }
    }

    showModal() {
        document.getElementById('confirmModal').style.display = 'flex';
    }

    hideModal() {
        document.getElementById('confirmModal').style.display = 'none';
        this.pendingAction = null;
    }

    showLoading(show) {
        document.getElementById('historyLoading').style.display = show ? 'flex' : 'none';
        document.getElementById('historyContent').style.display = show ? 'none' : 'block';
    }

    showError(message) {
        // TODO: Implémenter affichage d'erreur
        console.error(message);
    }

    formatDate(dateStr) {
        const date = new Date(dateStr);
        const today = new Date();
        const yesterday = new Date(today);
        yesterday.setDate(today.getDate() - 1);

        if (dateStr === today.toISOString().split('T')[0]) {
            return "Aujourd'hui";
        } else if (dateStr === yesterday.toISOString().split('T')[0]) {
            return "Hier";
        } else {
            return date.toLocaleDateString('fr-FR', {
                weekday: 'long',
                year: 'numeric',
                month: 'long',
                day: 'numeric'
            });
        }
    }
}

// Initialiser au chargement de la page
document.addEventListener('DOMContentLoaded', () => {
    new HistoryManager();
});