#!/bin/bash

# Script de démarrage automatique pour le système de détection d'objets colorés
# Usage: ./start.sh [--dev|--prod|--python-only]

set -e

# Couleurs pour l'affichage
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
NC='\033[0m' # No Color

# Configuration
PROJECT_NAME="Système de Détection d'Objets Colorés"
RUST_PORT=3000
FRONTEND_PORT=8080

echo -e "${PURPLE}🎯 $PROJECT_NAME${NC}"
echo -e "${PURPLE}${'='*50}${NC}"

# Fonction d'aide
show_help() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --dev          Mode développement (debug, logs détaillés)"
    echo "  --prod         Mode production (release, optimisé)"  
    echo "  --python-only  Lancer uniquement le script Python"
    echo "  --help         Afficher cette aide"
    echo ""
    echo "Exemples:"
    echo "  $0              # Mode développement par défaut"
    echo "  $0 --prod       # Mode production"
    echo "  $0 --python-only # Script Python standalone"
}

# Vérifier les prérequis
check_prerequisites() {
    echo -e "${BLUE}🔍 Vérification des prérequis...${NC}"
    
    # Vérifier Rust
    if ! command -v cargo &> /dev/null; then
        echo -e "${RED}❌ Rust/Cargo non trouvé${NC}"
        echo "Installer Rust: https://rustup.rs/"
        exit 1
    fi
    
    # Vérifier Python (optionnel)
    if ! command -v python3 &> /dev/null; then
        echo -e "${YELLOW}⚠️ Python3 non trouvé (optionnel pour script standalone)${NC}"
    fi
    
    echo -e "${GREEN}✅ Prérequis vérifiés${NC}"
}

# Créer la structure de dossiers
create_structure() {
    echo -e "${BLUE}📁 Création de la structure...${NC}"
    
    mkdir -p frontend/assets/captures
    mkdir -p frontend/css
    mkdir -p frontend/js
    
    echo -e "${GREEN}✅ Structure créée${NC}"
}

# Installer les dépendances Python
install_python_deps() {
    if [ -f "requirements.txt" ] && command -v pip3 &> /dev/null; then
        echo -e "${BLUE}🐍 Installation des dépendances Python...${NC}"
        pip3 install -r requirements.txt
        echo -e "${GREEN}✅ Dépendances Python installées${NC}"
    fi
}

# Compiler le backend Rust
build_backend() {
    echo -e "${BLUE}🦀 Compilation du backend Rust...${NC}"
    
    cd backend
    
    if [ "$1" = "prod" ]; then
        echo -e "${YELLOW}🏗️ Compilation en mode release...${NC}"
        cargo build --release
        RUST_BINARY="./target/release/color-detection-backend"
    else
        echo -e "${YELLOW}🏗️ Compilation en mode debug...${NC}"
        cargo build
        RUST_BINARY="./target/debug/color-detection-backend"
    fi
    
    cd ..
    echo -e "${GREEN}✅ Backend compilé${NC}"
}

# Démarrer le serveur backend
start_backend() {
    echo -e "${BLUE}🚀 Démarrage du serveur backend...${NC}"
    
    cd backend
    
    if [ "$1" = "prod" ]; then
        echo -e "${GREEN}🌐 Serveur en mode production sur http://127.0.0.1:$RUST_PORT${NC}"
        cargo run --release
    else
        echo -e "${GREEN}🌐 Serveur en mode développement sur http://127.0.0.1:$RUST_PORT${NC}"
        RUST_LOG=debug cargo run
    fi
}

# Démarrer le script Python standalone
start_python_standalone() {
    echo -e "${BLUE}🐍 Démarrage du script Python standalone...${NC}"
    
    if [ ! -f "detection_script.py" ]; then
        echo -e "${RED}❌ Script detection_script.py non trouvé${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}🎥 Script Python démarré${NC}"
    echo -e "${YELLOW}💡 Utilisez --help pour voir les options${NC}"
    
    python3 detection_script.py "$@"
}

# Ouvrir le navigateur (optionnel)
open_browser() {
    if command -v xdg-open &> /dev/null; then
        xdg-open "http://127.0.0.1:$RUST_PORT" &
    elif command -v open &> /dev/null; then
        open "http://127.0.0.1:$RUST_PORT" &
    else
        echo -e "${YELLOW}💻 Ouvrez manuellement: http://127.0.0.1:$RUST_PORT${NC}"
    fi
}

# Nettoyage en cas d'interruption
cleanup() {
    echo -e "\n${YELLOW}🛑 Arrêt en cours...${NC}"
    
    # Tuer les processus enfants
    jobs -p | xargs -r kill
    
    echo -e "${GREEN}✅ Nettoyage terminé${NC}"
    exit 0
}

# Gestion des signaux
trap cleanup SIGINT SIGTERM

# Fonction principale
main() {
    local mode="dev"
    local python_only=false
    
    # Parser les arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            --dev)
                mode="dev"
                shift
                ;;
            --prod)
                mode="prod" 
                shift
                ;;
            --python-only)
                python_only=true
                shift
                ;;
            --help)
                show_help
                exit 0
                ;;
            *)
                echo -e "${RED}❌ Option inconnue: $1${NC}"
                show_help
                exit 1
                ;;
        esac
    done
    
    # Script Python uniquement
    if [ "$python_only" = true ]; then
        start_python_standalone
        return
    fi
    
    # Démarrage normal avec backend Rust
    check_prerequisites
    create_structure
    
    # Installation optionnelle des dépendances Python
    if [ "$mode" = "dev" ]; then
        install_python_deps
    fi
    
    # Compilation et démarrage
    build_backend "$mode"
    
    echo -e "${GREEN}🎯 Système prêt !${NC}"
    echo -e "${BLUE}📍 Adresses importantes:${NC}"
    echo -e "  • Interface web: http://127.0.0.1:$RUST_PORT"
    echo -e "  • API: http://127.0.0.1:$RUST_PORT/api"
    echo -e "  • Page détection: http://127.0.0.1:$RUST_PORT/"
    echo -e "  • Page connexion: http://127.0.0.1:$RUST_PORT/login.html"
    echo -e "  • Page historique: http://127.0.0.1:$RUST_PORT/history.html"
    echo ""
    echo -e "${YELLOW}🔑 Identifiants par défaut:${NC}"
    echo -e "  • Utilisateur: admin"
    echo -e "  • Mot de passe: admin123"
    echo ""
    echo -e "${PURPLE}🎮 Instructions:${NC}"
    echo -e "  1. Ouvrez http://127.0.0.1:$RUST_PORT dans votre navigateur"
    echo -e "  2. Cliquez sur 'Démarrer Caméra'"
    echo -e "  3. Présentez des objets rouges, verts ou bleus"
    echo -e "  4. Observez les détections automatiques"
    echo ""
    echo -e "${RED}🛑 Appuyez sur Ctrl+C pour arrêter${NC}"
    
    # Ouvrir le navigateur automatiquement en mode dev
    if [ "$mode" = "dev" ]; then
        sleep 2
        open_browser &
    fi
    
    # Démarrer le serveur
    start_backend "$mode"
}

# Point d'entrée
main "$@"