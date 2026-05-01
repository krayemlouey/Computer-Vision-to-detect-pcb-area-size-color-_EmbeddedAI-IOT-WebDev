// enhanced-auth.js - Système d'authentification renforcé
class EnhancedAuthGuard {
    constructor() {
        this.sessionTimeout = 30 * 60 * 1000; // 30 minutes
        this.checkInterval = 30 * 1000; // Vérifier chaque 30 secondes
        this.maxRetries = 3;
        this.retryCount = 0;
        
        this.initGuard();
    }
    
    initGuard() {
        const currentPage = window.location.pathname.split('/').pop();
        const protectedPages = ['history.html'];
        
        // Nettoyer les sessions expirées au démarrage
        this.cleanupExpiredSessions();
        
        // Vérifier si la page nécessite une authentification
        if (protectedPages.includes(currentPage)) {
            this.enforceAuthentication();
        }
        
        // Démarrer la surveillance de session
        this.startSessionMonitoring();
        
        // Gestion de la visibilité de la page
        this.handlePageVisibility();
    }
    
    async enforceAuthentication() {
        const isAuth = await this.verifyAuthentication();
        
        if (!isAuth) {
            console.log('🔒 Accès refusé - Authentification requise');
            this.redirectToLogin();
            return false;
        }
        
        console.log('✅ Authentification vérifiée');
        return true;
    }
    
    async verifyAuthentication() {
        // Vérification locale d'abord
        if (!this.isLocallyAuthenticated()) {
            return false;
        }
        
        // Vérification côté serveur
        try {
            const response = await fetch('/api/verify-auth', {
                method: 'GET',
                headers: this.getAuthHeaders()
            });
            
            if (response.ok) {
                const data = await response.json();
                if (data.authenticated) {
                    this.retryCount = 0; // Reset retry count on success
                    return true;
                }
            }
            
            throw new Error('Authentification serveur échouée');
            
        } catch (error) {
            console.error('❌ Erreur vérification auth:', error);
            
            // Implement retry mechanism
            if (this.retryCount < this.maxRetries) {
                this.retryCount++;
                console.log(`🔄 Tentative de reconnexion ${this.retryCount}/${this.maxRetries}`);
                
                // Retry after a delay
                await new Promise(resolve => setTimeout(resolve, 2000 * this.retryCount));
                return this.verifyAuthentication();
            }
            
            // If all retries failed, logout
            this.logout();
            return false;
        }
    }
    
    isLocallyAuthenticated() {
        const token = this.getStoredToken();
        const loginTime = this.getStoredLoginTime();
        const username = this.getStoredUsername();
        
        if (!token || !loginTime || !username) {
            console.log('🔒 Données d\'authentification locales manquantes');
            return false;
        }
        
        // Vérifier l'expiration
        const elapsed = Date.now() - parseInt(loginTime);
        if (elapsed > this.sessionTimeout) {
            console.log('🔒 Session locale expirée');
            return false;
        }
        
        // Vérifier le format du token JWT (basique)
        if (!this.isValidTokenFormat(token)) {
            console.log('🔒 Format de token invalide');
            return false;
        }
        
        return true;
    }
    
    isValidTokenFormat(token) {
        if (!token || typeof token !== 'string') return false;
        
        const parts = token.split('.');
        if (parts.length !== 3) return false;
        
        try {
            // Vérifier que les parties sont en base64 valide
            atob(parts[0]); // header
            const payload = JSON.parse(atob(parts[1])); // payload
            
            // Vérifier l'expiration du token
            if (payload.exp && payload.exp < Math.floor(Date.now() / 1000)) {
                console.log('🔒 Token JWT expiré');
                return false;
            }
            
            return true;
        } catch (error) {
            console.error('❌ Erreur validation token:', error);
            return false;
        }
    }
    
    async login(username, password) {
        try {
            console.log('🔐 Tentative de connexion...');
            
            const response = await fetch('/api/login', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify({
                    username: username.trim(),
                    password: password
                })
            });

            const data = await response.json();

            if (response.ok && data.success && data.token) {
                // Stocker les informations d'authentification de manière sécurisée
                this.storeAuthData(data.token, username);
                
                console.log('✅ Connexion réussie');
                return { success: true, data };
            } else {
                throw new Error(data.message || 'Échec de connexion');
            }
        } catch (error) {
            console.error('❌ Erreur de connexion:', error);
            throw error;
        }
    }
    
    storeAuthData(token, username) {
        const loginTime = Date.now().toString();
        
        // Utiliser sessionStorage pour plus de sécurité
        sessionStorage.setItem('authToken', token);
        sessionStorage.setItem('username', username);
        sessionStorage.setItem('loginTime', loginTime);
        
        // Nettoyer localStorage au cas où il y aurait des données anciennes
        localStorage.removeItem('authToken');
        localStorage.removeItem('username');
        localStorage.removeItem('loginTime');
    }
    
    getStoredToken() {
        return sessionStorage.getItem('authToken') || localStorage.getItem('authToken');
    }
    
    getStoredUsername() {
        return sessionStorage.getItem('username') || localStorage.getItem('username') || 'Utilisateur';
    }
    
    getStoredLoginTime() {
        return sessionStorage.getItem('loginTime') || localStorage.getItem('loginTime');
    }
    
    getAuthHeaders() {
        const token = this.getStoredToken();
        return token ? { 'Authorization': `Bearer ${token}` } : {};
    }
    
    logout() {
        console.log('🚪 Déconnexion...');
        
        // Nettoyer toutes les données
        sessionStorage.clear();
        localStorage.clear();
        
        // Nettoyer les cookies
        this.clearAllCookies();
        
        // Rediriger vers login
        this.redirectToLogin();
    }
    
    clearAllCookies() {
        document.cookie.split(";").forEach(function(c) { 
            document.cookie = c.replace(/^ +/, "").replace(/=.*/, "=;expires=" + new Date().toUTCString() + ";path=/"); 
        });
    }
    
    redirectToLogin() {
        const currentPage = window.location.pathname.split('/').pop();
        if (currentPage !== 'login.html') {
            console.log('↩️ Redirection vers login');
            
            // Sauvegarder la page de destination
            sessionStorage.setItem('returnUrl', window.location.href);
            
            window.location.href = 'login.html';
        }
    }
    
    handleReturnUrl() {
        const returnUrl = sessionStorage.getItem('returnUrl');
        if (returnUrl && returnUrl.includes('history.html')) {
            sessionStorage.removeItem('returnUrl');
            window.location.href = returnUrl;
            return true;
        }
        return false;
    }
    
    startSessionMonitoring() {
        // Vérification périodique de la session
        const monitoringInterval = setInterval(async () => {
            const isAuth = await this.verifyAuthentication();
            
            if (!isAuth) {
                clearInterval(monitoringInterval);
                this.showSessionExpiredAlert();
            } else {
                // Rafraîchir le timestamp de la session
                this.extendSession();
            }
        }, this.checkInterval);
        
        // Nettoyer l'intervalle avant la fermeture de la page
        window.addEventListener('beforeunload', () => {
            clearInterval(monitoringInterval);
        });
    }
    
    handlePageVisibility() {
        document.addEventListener('visibilitychange', async () => {
            if (document.visibilityState === 'visible') {
                // Page devient visible - vérifier l'auth
                const currentPage = window.location.pathname.split('/').pop();
                const protectedPages = ['history.html'];
                
                if (protectedPages.includes(currentPage)) {
                    const isAuth = await this.verifyAuthentication();
                    if (!isAuth) {
                        this.showSessionExpiredAlert();
                    }
                }
            }
        });
    }
    
    showSessionExpiredAlert() {
        const alertDiv = document.createElement('div');
        alertDiv.style.cssText = `
            position: fixed;
            top: 0;
            left: 0;
            right: 0;
            bottom: 0;
            background: rgba(0,0,0,0.8);
            z-index: 100000;
            display: flex;
            align-items: center;
            justify-content: center;
            font-family: Arial, sans-serif;
        `;
        
        alertDiv.innerHTML = `
            <div style="background: white; padding: 30px; border-radius: 10px; text-align: center; max-width: 400px;">
                <h3 style="color: #dc3545; margin-bottom: 15px;">⚠️ Session Expirée</h3>
                <p style="margin-bottom: 20px; color: #666;">
                    Votre session a expiré pour des raisons de sécurité. 
                    Vous allez être redirigé vers la page de connexion.
                </p>
                <button onclick="this.parentElement.parentElement.remove(); window.location.href='login.html';" 
                        style="background: #007bff; color: white; border: none; padding: 10px 20px; border-radius: 5px; cursor: pointer;">
                    Se reconnecter
                </button>
            </div>
        `;
        
        document.body.appendChild(alertDiv);
        
        // Auto-redirect après 5 secondes
        setTimeout(() => {
            if (alertDiv.parentNode) {
                alertDiv.parentNode.removeChild(alertDiv);
            }
            this.redirectToLogin();
        }, 5000);
    }
    
    extendSession() {
        if (this.isLocallyAuthenticated()) {
            sessionStorage.setItem('loginTime', Date.now().toString());
        }
    }
    
    cleanupExpiredSessions() {
        const loginTime = this.getStoredLoginTime();
        if (loginTime) {
            const elapsed = Date.now() - parseInt(loginTime);
            if (elapsed > this.sessionTimeout) {
                console.log('🧹 Nettoyage session expirée');
                this.logout();
            }
        }
    }
    
    // Méthodes utilitaires pour les autres scripts
    async makeAuthenticatedRequest(url, options = {}) {
        const headers = {
            'Content-Type': 'application/json',
            ...this.getAuthHeaders(),
            ...options.headers
        };

        try {
            const response = await fetch(url, {
                ...options,
                headers
            });

            // Gérer l'expiration automatique du token
            if (response.status === 401) {
                console.log('🔒 Token expiré côté serveur');
                this.logout();
                throw new Error('Session expirée');
            }

            return response;
        } catch (error) {
            if (error.message === 'Session expirée') {
                throw error;
            }
            throw new Error('Erreur de connexion au serveur');
        }
    }
}

// Initialiser le système d'authentification
const authGuard = new EnhancedAuthGuard();

// Exposer globalement pour les autres scripts
window.AuthGuard = authGuard;

// Interface de compatibilité avec vos scripts existants
window.AuthManager = {
    isAuthenticated: () => authGuard.isLocallyAuthenticated(),
    getToken: () => authGuard.getStoredToken(),
    getUsername: () => authGuard.getStoredUsername(),
    getLoginTime: () => {
        const time = authGuard.getStoredLoginTime();
        return time ? new Date(parseInt(time)) : null;
    },
    logout: () => authGuard.logout(),
    forceLogout: () => authGuard.logout(),
    getAuthHeaders: () => authGuard.getAuthHeaders(),
    makeAuthenticatedRequest: (url, options) => authGuard.makeAuthenticatedRequest(url, options)
};