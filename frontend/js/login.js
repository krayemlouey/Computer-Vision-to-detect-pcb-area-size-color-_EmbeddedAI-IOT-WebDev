class LoginManager {
    constructor() {
        this.initEventListeners();
        this.checkExistingAuth();
    }

    initEventListeners() {
        const loginForm = document.getElementById('loginForm');
        loginForm.addEventListener('submit', (e) => this.handleLogin(e));
        
        // Auto-focus sur le champ username
        document.getElementById('username').focus();
    }

    checkExistingAuth() {
        const token = localStorage.getItem('authToken');
        if (token && this.isValidToken(token)) {
            // Rediriger vers la page historique si déjà connecté
            window.location.href = 'history.html';
        }
    }

    async handleLogin(event) {
        event.preventDefault();
        
        const username = document.getElementById('username').value.trim();
        const password = document.getElementById('password').value;
        
        if (!username || !password) {
            this.showMessage('Veuillez remplir tous les champs', 'error');
            return;
        }

        this.setLoadingState(true);

        try {
            const response = await fetch('/api/login', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify({
                    username: username,
                    password: password
                })
            });

            const data = await response.json();

            if (response.ok && data.success) {
                // Sauvegarder le token
                localStorage.setItem('authToken', data.token);
                localStorage.setItem('username', username);
                
                this.showMessage('Connexion réussie ! Redirection...', 'success');
                
                // Redirection après un court délai
                setTimeout(() => {
                    window.location.href = 'history.html';
                }, 1500);
                
            } else {
                this.showMessage(data.message || 'Erreur de connexion', 'error');
            }
        } catch (error) {
            console.error('Erreur de connexion:', error);
            this.showMessage('Erreur de connexion au serveur', 'error');
        } finally {
            this.setLoadingState(false);
        }
    }

    setLoadingState(loading) {
        const loginBtn = document.querySelector('.login-btn');
        const btnText = document.getElementById('loginBtnText');
        const spinner = document.getElementById('loginSpinner');
        
        loginBtn.disabled = loading;
        
        if (loading) {
            btnText.style.display = 'none';
            spinner.style.display = 'inline-block';
        } else {
            btnText.style.display = 'inline';
            spinner.style.display = 'none';
        }
    }

    showMessage(message, type) {
        const messageDiv = document.getElementById('loginMessage');
        messageDiv.textContent = message;
        messageDiv.className = `message ${type}`;
        messageDiv.style.display = 'block';
        
        // Masquer le message après 5 secondes si c'est une erreur
        if (type === 'error') {
            setTimeout(() => {
                messageDiv.style.display = 'none';
            }, 5000);
        }
    }

    isValidToken(token) {
        if (!token) return false;
        
        try {
            // Vérification basique du format JWT
            const parts = token.split('.');
            if (parts.length !== 3) return false;
            
            // Décoder le payload pour vérifier l'expiration
            const payload = JSON.parse(atob(parts[1]));
            const currentTime = Math.floor(Date.now() / 1000);
            
            return payload.exp > currentTime;
        } catch (error) {
            return false;
        }
    }
}

// Utilitaire pour gérer l'authentification
class AuthManager {
    static getToken() {
        return localStorage.getItem('authToken');
    }

    static isAuthenticated() {
        const token = this.getToken();
        if (!token) return false;
        
        try {
            const parts = token.split('.');
            if (parts.length !== 3) return false;
            
            const payload = JSON.parse(atob(parts[1]));
            const currentTime = Math.floor(Date.now() / 1000);
            
            return payload.exp > currentTime;
        } catch (error) {
            return false;
        }
    }

    static logout() {
        localStorage.removeItem('authToken');
        localStorage.removeItem('username');
        window.location.href = 'login.html';
    }

    static getAuthHeaders() {
        const token = this.getToken();
        return token ? { 'Authorization': `Bearer ${token}` } : {};
    }

    static async makeAuthenticatedRequest(url, options = {}) {
        const headers = {
            'Content-Type': 'application/json',
            ...this.getAuthHeaders(),
            ...options.headers
        };

        const response = await fetch(url, {
            ...options,
            headers
        });

        if (response.status === 401) {
            // Token expiré ou invalide
            this.logout();
            throw new Error('Session expirée');
        }

        return response;
    }
}

// Initialiser au chargement de la page
document.addEventListener('DOMContentLoaded', () => {
    new LoginManager();
});

// Exposer AuthManager globalement pour les autres scripts
window.AuthManager = AuthManager;