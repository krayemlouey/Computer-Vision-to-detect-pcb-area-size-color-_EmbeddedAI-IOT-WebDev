#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Service Flask de Détection pour le Système ESP32
Version corrigée avec configuration réseau automatique et gestion d'erreurs robuste
"""

from flask import Flask, Response, jsonify, render_template_string, request
from flask_socketio import SocketIO, emit
import requests
import base64
import json
import threading
import time
from datetime import datetime
import os
import cv2
from flask_cors import CORS
import jwt
import socket

# Import du service de détection
try:
    from detection_service import DetectionService
    DETECTION_SERVICE_AVAILABLE = True
except ImportError:
    print("[WARN] detection_service.py non trouvé - Mode simulation activé")
    DETECTION_SERVICE_AVAILABLE = False

# Configuration Flask
app = Flask(__name__)
app.secret_key = 'detection_service_secret_key_esp32_2024'
socketio = SocketIO(app, cors_allowed_origins="*", ping_timeout=60, ping_interval=25)
CORS(app, origins=["*"])

# Variables globales
RUST_API_URL = None
DETECTION_COOLDOWN = 3.0

def get_local_ip():
    """Détecte automatiquement l'IP locale"""
    try:
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as s:
            s.connect(("8.8.8.8", 80))
            local_ip = s.getsockname()[0]
            return local_ip
    except:
        try:
            hostname = socket.gethostname()
            local_ip = socket.gethostbyname(hostname)
            if local_ip != "127.0.0.1":
                return local_ip
        except:
            pass
    return "127.0.0.1"

def find_rust_server():
    """Cherche le serveur Rust sur différents ports et IPs"""
    local_ip = get_local_ip()
    
    ips_to_test = [
        local_ip,
        "127.0.0.1",
        "192.168.1.50",
    ]
    
    ports_to_test = [3001, 3002, 3003, 3004, 3005]
    
    print(f"[SEARCH] Recherche du serveur Rust...")
    print(f"   IP locale détectée: {local_ip}")
    
    for ip in ips_to_test:
        for port in ports_to_test:
            try:
                url = f"http://{ip}:{port}/api/esp32/ping"
                print(f"   Test: {url}")
                
                response = requests.get(url, timeout=2)
                if response.status_code == 200:
                    data = response.json()
                    if "pong" in data.get("status", ""):
                        print(f"[OK] Serveur Rust trouvé: {ip}:{port}")
                        return f"http://{ip}:{port}/api"
            except:
                continue
    
    print("[ERROR] Serveur Rust non trouvé - utilisation de la configuration par défaut")
    return f"http://{local_ip}:3001/api"

# Configuration système avec détection automatique
local_ip = get_local_ip()
RUST_API_URL = find_rust_server()

print(f"[INFO] Configuration Flask:")
print(f"   IP locale: {local_ip}")
print(f"   Backend Rust: {RUST_API_URL}")

class DynamicColorMapping:
    """Gestionnaire dynamique des mappings de couleurs depuis l'API Rust"""
    
    def __init__(self, rust_api_url=None):
        global RUST_API_URL
        self.rust_api_url = rust_api_url or RUST_API_URL
        self.color_mapping = self._load_default_mapping()
        self.last_update = time.time()
        self.connection_attempts = 0
        self.max_attempts = 5
        self._refresh_mappings()
    
    def _load_default_mapping(self):
        """Mapping par défaut en cas d'erreur de connexion"""
        return {
            'Rouge': {'g_id': '1001', 'type_name': 'Carte microchip'},
            'Vert': {'g_id': '1002', 'type_name': 'Carte personnalisée'},
            'Bleu': {'g_id': '1003', 'type_name': 'STM32'},
            'Jaune': {'g_id': '1004', 'type_name': 'Composant jaune'},
            'Noir': {'g_id': '1005', 'type_name': 'Composant noir'},
        }
    
    def _refresh_mappings(self):
        """Récupère les mappings depuis l'API Rust avec retry"""
        if self.connection_attempts >= self.max_attempts:
            print("[WARN] Trop de tentatives échouées, utilisation des mappings par défaut")
            return
            
        try:
            response = requests.get(f"{self.rust_api_url}/color-mappings", timeout=5)
            if response.status_code == 200:
                mappings = response.json()
                
                # Convertir en format utilisable
                new_mapping = {}
                for mapping in mappings:
                    color_key = mapping['color'].capitalize()
                    new_mapping[color_key] = {
                        'g_id': mapping['g_id'],
                        'type_name': mapping['type_name']
                    }
                
                if new_mapping:
                    self.color_mapping = new_mapping
                    self.last_update = time.time()
                    self.connection_attempts = 0
                    print(f"[OK] Mappings mis à jour: {len(new_mapping)} couleurs")
                
        except requests.exceptions.RequestException as e:
            self.connection_attempts += 1
            print(f"[WARN] Erreur récupération mappings (tentative {self.connection_attempts}): {e}")
            
            if self.connection_attempts >= 3:
                print("[SEARCH] Recherche d'un nouveau serveur Rust...")
                global RUST_API_URL
                RUST_API_URL = find_rust_server()
                self.rust_api_url = RUST_API_URL
                self.connection_attempts = 0
    
    def get_mapping(self, color_name):
        """Récupère le mapping pour une couleur donnée"""
        if time.time() - self.last_update > 30:
            self._refresh_mappings()
        
        return self.color_mapping.get(color_name.capitalize(), {
            'g_id': '1000', 
            'type_name': f'Inconnu ({color_name})'
        })
    
    def get_all_mappings(self):
        """Retourne tous les mappings actuels"""
        if time.time() - self.last_update > 30:
            self._refresh_mappings()
        return self.color_mapping.copy()
    
    def force_refresh(self):
        """Force le rafraîchissement des mappings"""
        old_mappings = self.color_mapping.copy()
        self._refresh_mappings()
        
        if old_mappings != self.color_mapping:
            socketio.emit('mappings_updated', {
                'mappings': self.color_mapping,
                'timestamp': datetime.now().isoformat(),
                'source': 'ESP32_UPDATE'
            })
        
        return self.color_mapping

# Instance globale du gestionnaire de mappings
dynamic_mapping = DynamicColorMapping()

class MockDetectionService:
    """Service de détection simulé pour les tests"""
    
    def __init__(self):
        self.camera_active = False
        self.frame_count = 0
        self.current_frame = None
        
    def start_camera(self):
        self.camera_active = True
        print("[INFO] Service de détection simulé démarré")
        return True
        
    def stop_camera(self):
        self.camera_active = False
        print("[INFO] Service de détection simulé arrêté")
        
    def get_status(self):
        return {
            "camera_active": self.camera_active,
            "frame_count": self.frame_count,
            "detection_service": "mock"
        }
        
    def generate_frames(self):
        """Génère des frames de test"""
        while True:
            if self.camera_active:
                frame = cv2.imread('test_frame.jpg') if os.path.exists('test_frame.jpg') else None
                if frame is None:
                    frame = 255 * cv2.ones((480, 640, 3), dtype='uint8')
                    cv2.putText(frame, 'CAMERA SIMULATION', (50, 240), 
                               cv2.FONT_HERSHEY_SIMPLEX, 2, (0, 0, 255), 3)
                    cv2.putText(frame, f'Frame: {self.frame_count}', (50, 300), 
                               cv2.FONT_HERSHEY_SIMPLEX, 1, (0, 255, 0), 2)
                
                self.frame_count += 1
                ret, buffer = cv2.imencode('.jpg', frame)
                if ret:
                    yield (b'--frame\r\n'
                           b'Content-Type: image/jpeg\r\n\r\n' + buffer.tobytes() + b'\r\n')
            time.sleep(0.1)

class FlaskDetectionService:
    """Service principal de détection Flask avec intégration ESP32"""
    
    def __init__(self):
        if DETECTION_SERVICE_AVAILABLE:
            self.detection_service = DetectionService(socketio=socketio)
        else:
            self.detection_service = MockDetectionService()
            
        self.last_detections = {}
        self.detection_monitoring_thread = None
        self.is_monitoring = False
        self.stats = {
            'total_detections': 0,
            'esp32_updates': 0,
            'last_esp32_ping': None,
            'rust_connection_status': 'unknown'
        }
        
        print("🚀 Service Flask de détection ESP32 initialisé")
        self._test_rust_connection()
    
    def _test_rust_connection(self):
        """Test de la connexion Rust au démarrage"""
        try:
            response = requests.get(f"{RUST_API_URL}/esp32/ping", timeout=5)
            if response.status_code == 200:
                self.stats['rust_connection_status'] = 'connected'
                self.stats['last_esp32_ping'] = datetime.now().isoformat()
                print(f"[OK] Connexion Rust établie: {RUST_API_URL}")
            else:
                self.stats['rust_connection_status'] = 'error'
                print(f"[ERROR] Serveur Rust répond avec status: {response.status_code}")
        except Exception as e:
            self.stats['rust_connection_status'] = 'disconnected'
            print(f"[ERROR] Impossible de se connecter au serveur Rust: {e}")
    
    def start_camera(self):
        """Démarre la caméra via le service de détection"""
        try:
            success = self.detection_service.start_camera()
            
            if success:
                self._start_detection_monitoring()
                print("[INFO] Caméra démarrée avec succès")
                return True
            else:
                print("[ERROR] Échec du démarrage de la caméra")
                return False
                
        except Exception as e:
            print(f"[ERROR] Erreur lors du démarrage de la caméra: {e}")
            return False
    
    def stop_camera(self):
        """Arrête la caméra"""
        try:
            self._stop_detection_monitoring()
            self.detection_service.stop_camera()
            print("[INFO] Caméra arrêtée")
            return True
            
        except Exception as e:
            print(f"[ERROR] Erreur lors de l'arrêt de la caméra: {e}")
            return False
    
    def _start_detection_monitoring(self):
        """Démarre le thread de monitoring des détections"""
        if not self.is_monitoring:
            self.is_monitoring = True
            self.detection_monitoring_thread = threading.Thread(
                target=self._detection_monitor_worker,
                daemon=True
            )
            self.detection_monitoring_thread.start()
            print("[SEARCH] Monitoring des détections démarré")
    
    def _stop_detection_monitoring(self):
        """Arrête le monitoring des détections"""
        self.is_monitoring = False
        if self.detection_monitoring_thread and self.detection_monitoring_thread.is_alive():
            self.detection_monitoring_thread.join(timeout=2.0)
        print("🔍 Monitoring des détections arrêté")
    
    def _detection_monitor_worker(self):
        """Worker qui surveille les détections et les envoie via WebSocket"""
        print("🔄 Worker de monitoring des détections démarré")
        
        while self.is_monitoring:
            try:
                if hasattr(self.detection_service, 'get_current_frame'):
                    frame = self.detection_service.get_current_frame()
                    
                    if frame is not None:
                        if hasattr(self.detection_service, 'process_frame'):
                            validated_contours, tracked_centroids = self.detection_service.process_frame(frame)
                            
                            if validated_contours:
                                for color_name, (contour, (cx, cy)) in validated_contours.items():
                                    current_time = time.time()
                                    last_detection = self.last_detections.get(color_name, 0)
                                    
                                    if current_time - last_detection > DETECTION_COOLDOWN:
                                        area = cv2.contourArea(contour)
                                        
                                        detection_data = {
                                            'color': color_name,
                                            'centroid': [int(cx), int(cy)],
                                            'area': int(area),
                                            'timestamp': datetime.now().isoformat()
                                        }
                                        
                                        socketio.emit('detection_update', detection_data)
                                        self._capture_and_send_detection(frame, color_name)
                                        
                                        self.last_detections[color_name] = current_time
                                        self.stats['total_detections'] += 1
                
                time.sleep(0.5)
                
            except Exception as e:
                print(f"[ERROR] Erreur dans le worker de monitoring: {e}")
                time.sleep(1.0)
        
        print("🔄 Worker de monitoring des détections arrêté")
    
    def _capture_and_send_detection(self, frame, color_name):
        """Capture et envoie une détection à l'API Rust avec mappings dynamiques"""
        global RUST_API_URL
        
        try:
            mapping = dynamic_mapping.get_mapping(color_name)
            
            _, buffer = cv2.imencode('.jpg', frame, [cv2.IMWRITE_JPEG_QUALITY, 90])
            image_base64 = base64.b64encode(buffer).decode('utf-8')
            image_data_url = f"data:image/jpeg;base64,{image_base64}"
            
            payload = {
                "g_id": mapping['g_id'],
                "type_name": mapping['type_name'],
                "color": color_name.lower(),
                "image_data": image_data_url
            }
            
            max_retries = 3
            for attempt in range(max_retries):
                try:
                    response = requests.post(
                        f"{RUST_API_URL}/detections",
                        json=payload,
                        headers={"Content-Type": "application/json"},
                        timeout=5
                    )
                    
                    if response.status_code == 200:
                        result = response.json()
                        print(f"[OK] Détection sauvegardée: {result.get('id', 'N/A')} - {mapping['type_name']}")
                        
                        socketio.emit('detection_saved', {
                            'id': result.get('id'),
                            'type_name': mapping['type_name'],
                            'color': color_name,
                            'status': 'success',
                            'g_id': mapping['g_id']
                        })
                        
                        self.stats['rust_connection_status'] = 'connected'
                        break
                    else:
                        print(f"[ERROR] Erreur API Rust: {response.status_code} - {response.text}")
                        if attempt == max_retries - 1:
                            self.stats['rust_connection_status'] = 'error'
                            
                except requests.exceptions.RequestException as e:
                    print(f"[ERROR] Tentative {attempt + 1} échouée: {e}")
                    if attempt == max_retries - 1:
                        self.stats['rust_connection_status'] = 'disconnected'
                        RUST_API_URL = find_rust_server()
                        dynamic_mapping.rust_api_url = RUST_API_URL
                    else:
                        time.sleep(1)
                
        except Exception as e:
            print(f"[ERROR] Erreur capture/envoi: {e}")
    
    def get_status(self):
        """Retourne le statut du service"""
        base_status = self.detection_service.get_status()
        base_status.update({
            'esp32_integration': True,
            'rust_api_url': RUST_API_URL,
            'monitoring_active': self.is_monitoring,
            'stats': self.stats,
            'mappings_count': len(dynamic_mapping.get_all_mappings()),
            'local_ip': get_local_ip()
        })
        return base_status
    
    def generate_frames(self):
        """Génère les frames pour le streaming"""
        return self.detection_service.generate_frames()

# Instance globale du service
flask_service = FlaskDetectionService()

def start_mapping_monitor():
    """Démarre la surveillance automatique des mappings ESP32"""
    def mapping_monitor():
        while True:
            try:
                time.sleep(30)
                old_mappings = dynamic_mapping.get_all_mappings()
                dynamic_mapping.force_refresh()
                new_mappings = dynamic_mapping.get_all_mappings()
                
                if old_mappings != new_mappings:
                    print("🔄 Changements détectés dans les mappings ESP32:")
                    for color, mapping in new_mappings.items():
                        if color not in old_mappings or old_mappings[color] != mapping:
                            print(f"  📌 {color}: {mapping['type_name']} ({mapping['g_id']})")
                    
                    flask_service.stats['esp32_updates'] += 1
                
            except Exception as e:
                print(f"[ERROR] Erreur monitoring mappings: {e}")
                time.sleep(60)
    
    monitor_thread = threading.Thread(target=mapping_monitor, daemon=True)
    monitor_thread.start()
    print("👁️ Surveillance des mappings ESP32 démarrée")

# === ROUTES FLASK ===

@app.route('/')
def index():
    """Page d'accueil avec interface ESP32"""
    local_ip = get_local_ip()
    
    html_template = """
    <!DOCTYPE html>
    <html>
    <head>
        <title>Service Flask - Détection ESP32</title>
        <meta charset="utf-8">
        <style>
            body { 
                font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif; 
                margin: 0; padding: 20px;
                background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
                color: white; min-height: 100vh;
            }
            .container { 
                max-width: 1200px; margin: 0 auto; 
                background: rgba(255,255,255,0.1); padding: 30px; 
                border-radius: 15px; backdrop-filter: blur(10px);
                box-shadow: 0 8px 32px rgba(31, 38, 135, 0.37);
            }
            .header { text-align: center; margin-bottom: 30px; }
            .header h1 { 
                font-size: 2.5em; margin: 0; 
                background: linear-gradient(45deg, #fff, #f0f0f0);
                -webkit-background-clip: text; -webkit-text-fill-color: transparent;
                background-clip: text; text-shadow: 2px 2px 4px rgba(0,0,0,0.3);
            }
            .network-info {
                background: rgba(40, 167, 69, 0.2); padding: 15px; border-radius: 10px;
                margin: 20px 0; border-left: 4px solid #28a745;
            }
            .esp32-badge {
                display: inline-block; padding: 8px 16px; 
                background: linear-gradient(45deg, #28a745, #20c997);
                border-radius: 20px; font-size: 0.9em; margin-top: 10px;
                box-shadow: 0 4px 15px rgba(40, 167, 69, 0.3);
            }
            .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 30px; }
            .panel { 
                background: rgba(255,255,255,0.1); padding: 25px; 
                border-radius: 15px; border: 1px solid rgba(255,255,255,0.2);
            }
            .panel h3 { margin-top: 0; color: rgba(255,255,255,0.9); }
            .video-container { 
                text-align: center; background: rgba(0,0,0,0.3);
                padding: 20px; border-radius: 10px; margin: 20px 0;
            }
            #video { 
                max-width: 100%; height: auto; 
                border: 3px solid rgba(255,255,255,0.3); border-radius: 10px;
            }
            .controls { display: flex; gap: 15px; justify-content: center; flex-wrap: wrap; }
            .btn { 
                padding: 12px 24px; border: none; border-radius: 25px; 
                cursor: pointer; font-size: 16px; font-weight: bold;
                transition: all 0.3s ease; text-transform: uppercase; letter-spacing: 1px;
            }
            .btn-primary { 
                background: linear-gradient(45deg, #4CAF50, #45a049); color: white; 
                box-shadow: 0 4px 15px rgba(76, 175, 80, 0.3);
            }
            .btn-primary:hover:not(:disabled) { 
                transform: translateY(-2px); box-shadow: 0 6px 20px rgba(76, 175, 80, 0.4);
            }
            .btn-danger { 
                background: linear-gradient(45deg, #f44336, #d32f2f); color: white; 
                box-shadow: 0 4px 15px rgba(244, 67, 54, 0.3);
            }
            .btn-danger:hover:not(:disabled) { 
                transform: translateY(-2px); box-shadow: 0 6px 20px rgba(244, 67, 54, 0.4);
            }
            .btn-success { 
                background: linear-gradient(45deg, #2196F3, #1976D2); color: white; 
                box-shadow: 0 4px 15px rgba(33, 150, 243, 0.3);
            }
            .btn:disabled { opacity: 0.5; cursor: not-allowed; transform: none !important; }
            .status { 
                padding: 15px; margin: 15px 0; border-radius: 10px; 
                text-align: center; font-weight: bold; border-left: 4px solid;
            }
            .status.success { 
                background: rgba(76, 175, 80, 0.2); border-left-color: #4CAF50; 
            }
            .status.error { 
                background: rgba(244, 67, 54, 0.2); border-left-color: #f44336; 
            }
            .status.info { 
                background: rgba(33, 150, 243, 0.2); border-left-color: #2196F3; 
            }
            .stats { 
                display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); 
                gap: 15px; margin: 20px 0; 
            }
            .stat-box { 
                background: rgba(255,255,255,0.1); padding: 20px; border-radius: 10px; 
                text-align: center; border: 1px solid rgba(255,255,255,0.2);
            }
            .stat-box h4 { 
                margin: 0 0 10px 0; color: rgba(255,255,255,0.8); 
                font-size: 14px; text-transform: uppercase; letter-spacing: 1px;
            }
            .stat-box div { font-size: 24px; font-weight: bold; color: white; }
            .detection-list { 
                max-height: 300px; overflow-y: auto; 
                background: rgba(0,0,0,0.2); border-radius: 10px; padding: 15px; 
            }
            .detection-item { 
                background: rgba(255,255,255,0.1); margin: 10px 0; 
                padding: 15px; border-radius: 8px; border-left: 4px solid #4CAF50;
                animation: slideIn 0.5s ease-out; 
            }
            @keyframes slideIn {
                from { transform: translateX(-100%); opacity: 0; }
                to { transform: translateX(0); opacity: 1; }
            }
            @media (max-width: 768px) { .grid { grid-template-columns: 1fr; } }
        </style>
    </head>
    <body>
        <div class="container">
            <div class="header">
                <h1>Service Flask - Détection de Couleurs</h1>
                <div class="esp32-badge">🎛️ Intégration ESP32 Active</div>
                
                <div class="network-info">
                    <strong>📡 Configuration Réseau:</strong><br>
                    IP Flask: """ + local_ip + """:5000<br>
                    Backend Rust: """ + RUST_API_URL + """<br>
                    Détection automatique: Activée
                </div>
            </div>
            
            <div id="status" class="status info">Service démarré - Test de connexion en cours...</div>
            
            <div class="grid">
                <div class="panel">
                    <h3>📊 Statistiques & Statut</h3>
                    <div class="stats">
                        <div class="stat-box">
                            <h4>Détections</h4>
                            <div id="detectionCount">0</div>
                        </div>
                        <div class="stat-box">
                            <h4>ESP32 Updates</h4>
                            <div id="esp32Count">0</div>
                        </div>
                        <div class="stat-box">
                            <h4>Caméra</h4>
                            <div id="cameraStatus">Inactive</div>
                        </div>
                        <div class="stat-box">
                            <h4>Backend Rust</h4>
                            <div id="rustStatus">Test...</div>
                        </div>
                    </div>
                </div>
                
                <div class="panel">
                    <h3>📷 Flux Vidéo en Temps Réel</h3>
                    <div class="video-container">
                        <img id="video" src="/video_feed" alt="Video Stream">
                    </div>
                    
                    <div class="controls">
                        <button id="startBtn" class="btn btn-primary" onclick="startCamera()">Démarrer</button>
                        <button id="stopBtn" class="btn btn-danger" onclick="stopCamera()" disabled>Arrêter</button>
                        <button id="testBtn" class="btn btn-success" onclick="testConnection()">Test API</button>
                    </div>
                </div>
            </div>
            
            <div class="panel">
                <h3>🔍 Détections Récentes</h3>
                <div id="detection-list" class="detection-list"></div>
            </div>
        </div>
        
        <script src="https://cdnjs.cloudflare.com/ajax/libs/socket.io/4.0.0/socket.io.js"></script>
        <script>
            const socket = io();
            let detectionCount = 0;
            
            socket.on('connect', function() {
                console.log('✅ Connecté au service Flask');
                updateStatus('Service Flask connecté - Test backend...', 'success');
                setTimeout(testConnection, 1000);
            });
            
            socket.on('detection_update', function(data) {
                detectionCount++;
                document.getElementById('detectionCount').textContent = detectionCount;
                addDetectionItem(data, 'Détecté');
                updateStatus(data.color + ' détecté!', 'success');
            });
            
            function addDetectionItem(data, status) {
                const list = document.getElementById('detection-list');
                const item = document.createElement('div');
                item.className = 'detection-item';
                item.innerHTML = '<strong>' + data.color.toUpperCase() + '</strong> - ' + status;
                list.insertBefore(item, list.firstChild);
                
                while (list.children.length > 5) {
                    list.removeChild(list.lastChild);
                }
            }
            
            function startCamera() {
                const startBtn = document.getElementById('startBtn');
                const stopBtn = document.getElementById('stopBtn');
                
                startBtn.innerHTML = 'Démarrage...';
                startBtn.disabled = true;
                
                fetch('/api/start_camera', {method: 'POST'})
                    .then(r => r.json())
                    .then(data => {
                        if (data.status === 'started') {
                            updateStatus('Caméra démarrée - Détection active', 'success');
                            document.getElementById('cameraStatus').textContent = 'Active';
                            stopBtn.disabled = false;
                            startBtn.innerHTML = 'Démarrer';
                            
                            document.getElementById('video').src = '/video_feed?t=' + Date.now();
                        } else {
                            throw new Error(data.message || 'Erreur démarrage');
                        }
                    })
                    .catch(e => {
                        updateStatus('Erreur démarrage: ' + e.message, 'error');
                        startBtn.disabled = false;
                        startBtn.innerHTML = 'Démarrer';
                    });
            }
            
            function stopCamera() {
                const startBtn = document.getElementById('startBtn');
                const stopBtn = document.getElementById('stopBtn');
                
                fetch('/api/stop_camera', {method: 'POST'})
                    .then(r => r.json())
                    .then(data => {
                        updateStatus('Caméra arrêtée', 'info');
                        document.getElementById('cameraStatus').textContent = 'Inactive';
                        startBtn.disabled = false;
                        stopBtn.disabled = true;
                    })
                    .catch(e => {
                        updateStatus('Erreur arrêt caméra', 'error');
                    });
            }
            
            function testConnection() {
                updateStatus('Test de connexion...', 'info');
                
                fetch('/api/test_rust_connection')
                    .then(r => r.json())
                    .then(data => {
                        if (data.rust_connected) {
                            updateStatus('Backend Rust connecté (' + data.types_count + ' types)', 'success');
                            document.getElementById('rustStatus').textContent = 'Connecté';
                        } else {
                            updateStatus('Backend Rust: ' + data.error, 'error');
                            document.getElementById('rustStatus').textContent = 'Déconnecté';
                        }
                    })
                    .catch(e => {
                        updateStatus('Test connexion échoué', 'error');
                        document.getElementById('rustStatus').textContent = 'Erreur';
                    });
            }
            
            function updateStatus(message, type) {
                const status = document.getElementById('status');
                status.textContent = message;
                status.className = 'status ' + type;
            }
            
            setInterval(() => {
                fetch('/api/status')
                    .then(r => r.json())
                    .then(data => {
                        document.getElementById('cameraStatus').textContent = 
                            data.camera_active ? 'Active' : 'Inactive';
                        
                        if (data.stats) {
                            document.getElementById('detectionCount').textContent = data.stats.total_detections || 0;
                            document.getElementById('esp32Count').textContent = data.stats.esp32_updates || 0;
                        }
                    })
                    .catch(e => console.log('Erreur récupération statut:', e));
            }, 3000);
            
            setTimeout(() => {
                testConnection();
            }, 1000);
        </script>
    </body>
    </html>
    """
    return render_template_string(html_template)

@app.route('/api/login', methods=['POST'])
def login():
    """Authentification utilisateur"""
    data = request.get_json()
    username = data.get("username")
    password = data.get("password")

    if username == "admin" and password == "admin123":
        token = jwt.encode(
            {
                "username": username,
                "exp": datetime.utcnow() + datetime.timedelta(minutes=30)
            },
            app.secret_key,
            algorithm="HS256"
        )
        return jsonify({"success": True, "token": token})
    else:
        return jsonify({"success": False, "message": "Identifiants incorrects"}), 401

@app.route('/video_feed')
def video_feed():
    """Stream vidéo en temps réel"""
    return Response(flask_service.generate_frames(),
                   mimetype='multipart/x-mixed-replace; boundary=frame')

@app.route('/api/start_camera', methods=['POST'])
def start_camera():
    """Démarre la caméra"""
    try:
        success = flask_service.start_camera()
        return jsonify({
            "status": "started" if success else "error",
            "message": "Caméra démarrée avec succès" if success else "Impossible de démarrer la caméra",
            "esp32_integration": True,
            "local_ip": get_local_ip(),
            "rust_backend": RUST_API_URL
        })
    except Exception as e:
        return jsonify({
            "status": "error",
            "message": f"Erreur: {str(e)}"
        }), 500

@app.route('/api/stop_camera', methods=['POST'])
def stop_camera():
    """Arrête la caméra"""
    try:
        flask_service.stop_camera()
        return jsonify({
            "status": "stopped",
            "message": "Caméra arrêtée",
            "esp32_integration": True
        })
    except Exception as e:
        return jsonify({
            "status": "error",
            "message": f"Erreur: {str(e)}"
        }), 500

@app.route('/api/status')
def get_status():
    """Statut du service Flask avec informations ESP32"""
    try:
        status = flask_service.get_status()
        return jsonify(status)
    except Exception as e:
        return jsonify({
            "camera_active": False,
            "frame_count": 0,
            "esp32_integration": True,
            "local_ip": get_local_ip(),
            "rust_api_url": RUST_API_URL,
            "error": str(e)
        })

@app.route('/api/test_rust_connection')
def test_rust_connection():
    """Test de la connexion au backend Rust ESP32"""
    global RUST_API_URL
    
    try:
        rust_connected = False
        types_count = 0
        
        test_urls = [
            RUST_API_URL,
            f"http://{get_local_ip()}:3001/api",
            "http://127.0.0.1:3001/api"
        ]
        
        for test_url in test_urls:
            try:
                response = requests.get(f"{test_url}/types", timeout=3)
                if response.status_code == 200:
                    rust_connected = True
                    types_count = len(response.json())
                    
                    if test_url != RUST_API_URL:
                        RUST_API_URL = test_url
                        dynamic_mapping.rust_api_url = RUST_API_URL
                    
                    break
                    
            except:
                continue
        
        flask_service.stats['rust_connection_status'] = 'connected' if rust_connected else 'disconnected'
        if rust_connected:
            flask_service.stats['last_esp32_ping'] = datetime.now().isoformat()
        
        return jsonify({
            "rust_connected": rust_connected,
            "rust_url": RUST_API_URL,
            "message": "Connexion OK" if rust_connected else "Aucun serveur Rust trouvé",
            "types_count": types_count,
            "local_ip": get_local_ip()
        })
        
    except Exception as e:
        flask_service.stats['rust_connection_status'] = 'error'
        return jsonify({
            "rust_connected": False,
            "rust_url": RUST_API_URL,
            "error": str(e),
            "local_ip": get_local_ip()
        })

@app.route('/api/detections')
def get_detections():
    """Récupération des détections depuis l'API Rust"""
    try:
        history_url = f"{RUST_API_URL}/history"
        response = requests.get(history_url, timeout=5)
        
        if response.status_code == 200:
            return jsonify(response.json())
        else:
            return jsonify({
                "data": [], 
                "error": f"Rust API erreur {response.status_code}",
                "pagination": {"total_items": 0}
            })
            
    except Exception as e:
        return jsonify({
            "data": [], 
            "error": str(e),
            "pagination": {"total_items": 0}
        })

@app.route('/api/counters')
def get_counters():
    """Endpoint compteurs pour compatibilité"""
    try:
        response = requests.get(f"{RUST_API_URL}/counters", timeout=5)
        if response.status_code == 200:
            return jsonify(response.json())
        else:
            return jsonify({
                "counters": {
                    "rouge": 0, "vert": 0, "bleu": 0, "jaune": 0, "noir": 0
                },
                "total": 0
            })
    except Exception as e:
        return jsonify({
            "counters": {
                "rouge": 0, "vert": 0, "bleu": 0, "jaune": 0, "noir": 0
            },
            "total": 0,
            "error": str(e)
        })

# === ÉVÉNEMENTS WEBSOCKET ===

@socketio.on('connect')
def handle_connect():
    print('Client WebSocket connecté')
    emit('connected', {
        'status': 'Flask Detection Service ESP32 connecté',
        'esp32_integration': True,
        'network_info': {
            'local_ip': get_local_ip(),
            'rust_url': RUST_API_URL
        },
        'timestamp': datetime.now().isoformat()
    })

@socketio.on('disconnect')
def handle_disconnect():
    print('Client WebSocket déconnecté')

@socketio.on('counter_update')
def handle_counter_update(data):
    """Gère les mises à jour de compteurs"""
    print(f"Mise à jour compteur reçue: {data}")
    # Retransmettre aux clients connectés
    emit('counter_update', data, broadcast=True)

# === GESTIONNAIRE D'ERREURS ===

@app.errorhandler(Exception)
def handle_exception(e):
    print(f"Erreur non gérée: {e}")
    return jsonify({
        "error": "Erreur interne du serveur ESP32",
        "details": str(e),
        "network_info": {
            "local_ip": get_local_ip(),
            "rust_url": RUST_API_URL
        }
    }), 500

@app.errorhandler(404)
def not_found(e):
    return jsonify({"error": "Endpoint non trouvé"}), 404

# === POINT D'ENTRÉE PRINCIPAL ===

if __name__ == '__main__':
    local_ip = get_local_ip()
    
    print("=" * 70)
    print("   SERVICE FLASK DE DETECTION - VERSION ESP32 CORRIGÉE")
    print("=" * 70)
    print(f"Interface web: http://{local_ip}:5000/")
    print(f"Stream vidéo: http://{local_ip}:5000/video_feed")
    print(f"API Flask: http://{local_ip}:5000/api/")
    print(f"Backend Rust: {RUST_API_URL}")
    print(f"Service de détection: {'Réel' if DETECTION_SERVICE_AVAILABLE else 'Simulation'}")
    print(f"IP locale détectée: {local_ip}")
    print("=" * 70)

    # Test de connexion au backend Rust
    print("\nTest de connectivité backend...")
    try:
        response = requests.get(f"{RUST_API_URL}/types", timeout=3)
        if response.status_code == 200:
            print("✅ Backend Rust accessible")
            
            mappings = dynamic_mapping.get_all_mappings()
            print(f"\nMappings couleur actuels ({len(mappings)}):")
            for color, mapping in mappings.items():
                print(f"   {color}: {mapping['type_name']} ({mapping['g_id']})")
        else:
            print(f"⚠️ Backend Rust répond avec status {response.status_code}")
    except Exception as e:
        print(f"⚠️ Backend Rust non accessible: {e}")
        print("   Le système va réessayer automatiquement de se connecter")
    
    print(f"\nService de détection: {'Réel' if DETECTION_SERVICE_AVAILABLE else 'Mode simulation'}")
    print(f"\nDémarrage du serveur Flask-SocketIO...")
    
    # Démarrer la surveillance des mappings ESP32
    start_mapping_monitor()

    try:
        socketio.run(
            app,
            host="0.0.0.0",
            port=5000,
            debug=False,
            allow_unsafe_werkzeug=True
        )
    except KeyboardInterrupt:
        print("\nArrêt demandé par l'utilisateur")
    except Exception as e:
        print(f"Erreur lors du démarrage: {e}")
    finally:
        flask_service.stop_camera()
        print("Service Flask ESP32 arrêté")