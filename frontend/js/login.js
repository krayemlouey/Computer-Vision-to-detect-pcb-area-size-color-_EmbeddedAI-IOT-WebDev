///login.js
class LoginManager {
    constructor() {
        this.initEventListeners();
        this.checkExistingAuth();
    }

    initEventListeners() {
        const loginForm = document.getElementById('loginForm');
        if (loginForm) {
            loginForm.addEventListener('submit', (e) => this.handleLogin(e));
        }
        
        // Auto-focus sur le champ username
        const usernameField = document.getElementById('username');
        if (usernameField) {
            usernameField.focus();
        }

        // Désactiver l'autocomplétion sur tous les champs de mot de passe
        const passwordFields = document.querySelectorAll('input[type="password"]');
        passwordFields.forEach(field => {
            field.setAttribute('autocomplete', 'off');
            field.setAttribute('autocapitalize', 'off');
            field.setAttribute('autocorrect', 'off');
            field.setAttribute('spellcheck', 'false');
        });

        // Désactiver l'autocomplétion sur le champ username aussi
        if (usernameField) {
            usernameField.setAttribute('autocomplete', 'off');
            usernameField.setAttribute('autocapitalize', 'off');
            usernameField.setAttribute('autocorrect', 'off');
            usernameField.setAttribute('spellcheck', 'false');
        }

        // Gestion de la touche Entrée
        document.addEventListener('keypress', (e) => {
            if (e.key === 'Enter' && !e.shiftKey) {
                const loginForm = document.getElementById('loginForm');
                if (loginForm && document.activeElement && 
                    (document.activeElement.id === 'username' || document.activeElement.id === 'password')) {
                    e.preventDefault();
                    this.handleLogin(e);
                }
            }
        });

        // Effacer les champs lors du focus (pour forcer la ressaisie)
        const formFields = document.querySelectorAll('#loginForm input');
        formFields.forEach(field => {
            field.addEventListener('focus', () => {
                field.value = '';
            });
        });
    }

    checkExistingAuth() {
        // Toujours nettoyer les données d'authentification au chargement de la page de login
        this.clearAuthData();
    }

    clearAuthData() {
        // Nettoyer toutes les données d'authentification
        localStorage.removeItem('authToken');
        localStorage.removeItem('username');
        localStorage.removeItem('loginTime');
        sessionStorage.clear();
        
        // Nettoyer aussi les cookies si présents
        document.cookie.split(";").forEach(function(c) { 
            document.cookie = c.replace(/^ +/, "").replace(/=.*/, "=;expires=" + new Date().toUTCString() + ";path=/"); 
        });
    }

    async handleLogin(event) {
        event.preventDefault();
        
        const username = document.getElementById('username')?.value?.trim();
        const password = document.getElementById('password')?.value;
        
        if (!username || !password) {
            this.showMessage('Veuillez remplir tous les champs', 'error');
            return;
        }

        console.log('🔐 Tentative de connexion:', username);
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
            console.log('📨 Réponse serveur:', data);

            if (response.ok && data.success) {
                // Sauvegarder le token avec timestamp
                localStorage.setItem('authToken', data.token);
                localStorage.setItem('username', username);
                localStorage.setItem('loginTime', Date.now().toString());
                
                console.log('✅ Connexion réussie, token sauvegardé');
                this.showMessage('Connexion réussie ! Redirection...', 'success');
                
                // Redirection après un court délai
                setTimeout(() => {
                    window.location.href = 'history.html';
                }, 1500);
                
            } else {
                console.log('❌ Échec connexion:', data.message);
                this.showMessage(data.message || 'Erreur de connexion', 'error');
                // Effacer les champs après échec
                this.clearFormFields();
            }
        } catch (error) {
            console.error('❌ Erreur de connexion:', error);
            this.showMessage('Erreur de connexion au serveur. Vérifiez que le serveur est démarré.', 'error');
            this.clearFormFields();
        } finally {
            this.setLoadingState(false);
        }
    }

    clearFormFields() {
        const usernameField = document.getElementById('username');
        const passwordField = document.getElementById('password');
        
        if (usernameField) usernameField.value = '';
        if (passwordField) passwordField.value = '';
        
        // Remettre le focus sur le username
        if (usernameField) usernameField.focus();
    }

    setLoadingState(loading) {
        const loginBtn = document.querySelector('.login-btn');
        const btnText = document.getElementById('loginBtnText');
        const spinner = document.getElementById('loginSpinner');
        const form = document.getElementById('loginForm');
        
        if (loginBtn) loginBtn.disabled = loading;
        
        if (btnText && spinner) {
            if (loading) {
                btnText.style.display = 'none';
                spinner.style.display = 'inline-block';
            } else {
                btnText.style.display = 'inline';
                spinner.style.display = 'none';
            }
        }
        
        // Désactiver les champs de saisie pendant le chargement
        if (form) {
            const inputs = form.querySelectorAll('input');
            inputs.forEach(input => {
                input.disabled = loading;
            });
        }
    }

    showMessage(message, type) {
        const messageDiv = document.getElementById('loginMessage');
        if (messageDiv) {
            messageDiv.innerHTML = ''; // Clear previous content
            messageDiv.textContent = message;
            messageDiv.className = `message ${type}`;
            messageDiv.style.display = 'block';
            
            // Animation d'apparition
            messageDiv.style.opacity = '0';
            messageDiv.style.transform = 'translateY(-10px)';
            setTimeout(() => {
                messageDiv.style.transition = 'all 0.3s ease';
                messageDiv.style.opacity = '1';
                messageDiv.style.transform = 'translateY(0)';
            }, 10);
            
            // Masquer le message après 5 secondes si c'est une erreur
            if (type === 'error') {
                setTimeout(() => {
                    if (messageDiv.className.includes('error')) {
                        messageDiv.style.opacity = '0';
                        setTimeout(() => {
                            messageDiv.style.display = 'none';
                        }, 300);
                    }
                }, 5000);
            }
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
            console.error('❌ Erreur validation token:', error);
            return false;
        }
    }
}

// Utilitaire avancé pour gérer l'authentification
class AuthManager {
    static getToken() {
        return localStorage.getItem('authToken');
    }

    static isAuthenticated() {
        const token = this.getToken();
        if (!token) {
            console.log('🔒 Aucun token trouvé');
            return false;
        }
        
        try {
            const parts = token.split('.');
            if (parts.length !== 3) {
                console.log('🔒 Format token invalide');
                this.logout();
                return false;
            }
            
            const payload = JSON.parse(atob(parts[1]));
            const currentTime = Math.floor(Date.now() / 1000);
            const isValid = payload.exp > currentTime;
            
            if (!isValid) {
                console.log('🔒 Token expiré');
                this.logout();
            }
            
            return isValid;
        } catch (error) {
            console.error('❌ Erreur vérification authentification:', error);
            this.logout();
            return false;
        }
    }

    static logout() {
        console.log('🚪 Déconnexion en cours...');
        
        // Nettoyer toutes les données d'authentification
        localStorage.removeItem('authToken');
        localStorage.removeItem('username');
        localStorage.removeItem('loginTime');
        sessionStorage.clear();
        
        // Nettoyer les cookies
        document.cookie.split(";").forEach(function(c) { 
            document.cookie = c.replace(/^ +/, "").replace(/=.*/, "=;expires=" + new Date().toUTCString() + ";path=/"); 
        });
        
        console.log('🧹 Données d\'authentification supprimées');
        
        // Rediriger vers la page de login
        if (!window.location.pathname.includes('login.html')) {
            console.log('↩️ Redirection vers login.html');
            window.location.href = 'login.html';
        }
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

        try {
            console.log(`📡 Requête authentifiée: ${options.method || 'GET'} ${url}`);
            
            const response = await fetch(url, {
                ...options,
                headers
            });

            // Gérer l'expiration du token
            if (response.status === 401) {
                console.log('🔒 Token expiré, déconnexion automatique');
                this.logout();
                throw new Error('Session expirée');
            }

            return response;
        } catch (error) {
            if (error.message === 'Session expirée') {
                throw error;
            }
            console.error('❌ Erreur requête authentifiée:', error);
            throw new Error('Erreur de connexion au serveur');
        }
    }

    static async verifyAuthentication() {
        if (!this.getToken()) {
            return false;
        }

        try {
            const response = await fetch('/api/verify', {
                headers: this.getAuthHeaders()
            });
            
            if (response.ok) {
                const data = await response.json();
                return data.authenticated === true;
            }
            
            return false;
        } catch (error) {
            console.error('❌ Erreur vérification auth:', error);
            return false;
        }
    }

    static getUsername() {
        return localStorage.getItem('username') || 'Utilisateur';
    }

    static getLoginTime() {
        const loginTime = localStorage.getItem('loginTime');
        return loginTime ? new Date(parseInt(loginTime)) : null;
    }

    static getTokenExpiration() {
        const token = this.getToken();
        if (!token) return null;
        
        try {
            const payload = JSON.parse(atob(token.split('.')[1]));
            return new Date(payload.exp * 1000);
        } catch (error) {
            console.error('❌ Erreur lecture expiration token:', error);
            return null;
        }
    }

    // Vérification périodique de l'authentification
    static startAuthCheck() {
        setInterval(() => {
            if (!this.isAuthenticated() && !window.location.pathname.includes('login.html')) {
                console.log('🔒 Session expirée, redirection vers login');
                this.logout();
            }
        }, 60000); // Vérifier toutes les minutes
    }

    // Fonction pour forcer la déconnexion complète
    static forceLogout() {
        // Nettoyer absolument tout
        localStorage.clear();
        sessionStorage.clear();
        
        // Nettoyer les cookies
        document.cookie.split(";").forEach(function(c) { 
            document.cookie = c.replace(/^ +/, "").replace(/=.*/, "=;expires=" + new Date().toUTCString() + ";path=/"); 
        });
        
        // Vider le cache du navigateur si possible
        if ('caches' in window) {
            caches.keys().then(function(names) {
                names.forEach(function(name) {
                    caches.delete(name);
                });
            });
        }
        
        // Redirection forcée
        window.location.replace('login.html');
    }
}

// Initialiser au chargement de la page
document.addEventListener('DOMContentLoaded', () => {
    // Vérifier si on est sur la page de login
    if (document.getElementById('loginForm')) {
        console.log('🔐 Initialisation LoginManager');
        new LoginManager();
    }
    
    // Démarrer la vérification périodique de l'authentification
    AuthManager.startAuthCheck();
});

// Exposer AuthManager globalement pour les autres scripts
window.AuthManager = AuthManager;