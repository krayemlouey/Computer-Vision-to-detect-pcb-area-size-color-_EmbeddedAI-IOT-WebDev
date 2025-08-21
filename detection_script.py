#!/usr/bin/env python3
"""
Script de détection d'objets colorés - Version standalone
Basé sur le code fourni avec intégration API
"""

from ultralytics import YOLO
import cv2
import numpy as np
import requests
import base64
import json
import time
from datetime import datetime
import argparse
import sys

class ColorDetectionSystem:
    def __init__(self, api_url="http://127.0.0.1:3000/api"):
        self.api_url = api_url
        
        # Charger le modèle YOLOv8 (optionnel, pour détection d'objets)
        try:
            self.model = YOLO("yolov8n.pt")
            print("✅ Modèle YOLOv8 chargé avec succès")
        except Exception as e:
            print(f"⚠️ Impossible de charger YOLOv8: {e}")
            self.model = None
        
        # Plages HSV pour rouge, vert, bleu (identiques au code original)
        self.colors = {
            'red': {
                'ranges': [(0, 120, 70, 10, 255, 255), (170, 120, 70, 180, 255, 255)],
                'contour_color': (0, 0, 255),
                'label': "Carte microchip",
                'g_id': "1001"
            },
            'green': {
                'ranges': [(36, 50, 70, 89, 255, 255)],
                'contour_color': (0, 255, 0),
                'label': "Carte personnalisée",
                'g_id': "1002"
            },
            'blue': {
                'ranges': [(90, 50, 70, 128, 255, 255)],
                'contour_color': (255, 0, 0),
                'label': "STM32",
                'g_id': "1003"
            }
        }
        
        # Dernières détections pour éviter les doublons
        self.last_detections = {color: 0 for color in self.colors.keys()}
        self.detection_cooldown = 2.0  # secondes entre détections
        
    def draw_color_contours(self, hsv_img, bgr_img, color_name, config):
        """Détecte et dessine les contours pour une couleur donnée"""
        mask = None
        
        if color_name == 'red':
            # Gestion spéciale pour le rouge (wraparound HSV)
            ranges = config['ranges']
            mask1 = cv2.inRange(hsv_img, np.array(ranges[0][:3]), np.array(ranges[0][3:]))
            mask2 = cv2.inRange(hsv_img, np.array(ranges[1][:3]), np.array(ranges[1][3:]))
            mask = cv2.bitwise_or(mask1, mask2)
        else:
            # Autres couleurs
            range_vals = config['ranges'][0]
            mask = cv2.inRange(hsv_img, np.array(range_vals[:3]), np.array(range_vals[3:]))
        
        # Nettoyage du masque (identique au code original)
        kernel = np.ones((5, 5), np.uint8)
        mask = cv2.morphologyEx(mask, cv2.MORPH_OPEN, kernel)
        mask = cv2.morphologyEx(mask, cv2.MORPH_DILATE, kernel)
        
        # Détection des contours
        contours, _ = cv2.findContours(mask, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_SIMPLE)
        
        detected = False
        if contours:
            # Filtrer les contours trop petits
            large_contours = [c for c in contours if cv2.contourArea(c) > 1000]
            
            if large_contours:
                detected = True
                cv2.drawContours(bgr_img, large_contours, -1, config['contour_color'], 2)
                
                # Afficher le texte au centre du plus grand contour
                largest_contour = max(large_contours, key=cv2.contourArea)
                M = cv2.moments(largest_contour)
                if M['m00'] != 0:
                    cx = int(M['m10'] / M['m00'])
                    cy = int(M['m01'] / M['m00'])
                    cv2.putText(bgr_img, config['label'], (cx - 50, cy), 
                              cv2.FONT_HERSHEY_SIMPLEX, 0.7, config['contour_color'], 2)
        
        return detected
    
    def capture_and_send_detection(self, frame, color_name, config):
        """Capture l'image et l'envoie à l'API"""
        current_time = time.time()
        
        # Vérifier le cooldown pour éviter les doublons
        if current_time - self.last_detections[color_name] < self.detection_cooldown:
            return
        
        self.last_detections[color_name] = current_time
        
        # Encoder l'image en base64
        _, buffer = cv2.imencode('.jpg', frame, [cv2.IMWRITE_JPEG_QUALITY, 85])
        image_base64 = base64.b64encode(buffer).decode('utf-8')
        image_data_url = f"data:image/jpeg;base64,{image_base64}"
        
        # Préparer les données pour l'API
        detection_data = {
            "g_id": config['g_id'],
            "type_name": config['label'],
            "color": color_name,
            "image_data": image_data_url
        }
        
        # Envoyer à l'API
        try:
            response = requests.post(
                f"{self.api_url}/detections",
                json=detection_data,
                headers={"Content-Type": "application/json"},
                timeout=5
            )
            
            if response.status_code == 200:
                detection = response.json()
                print(f"✅ Détection sauvegardée: {detection['id']} - {config['label']}")
            else:
                print(f"❌ Erreur API: {response.status_code}")
                
        except requests.exceptions.RequestException as e:
            print(f"🔌 Erreur connexion API: {e}")
    
def run_detection(self, detect_type='all', save_data=True):
    # Initialiser fps dès le début de la fonction
        fps = 0.0
        frame_count = 0
        start_time = time.time()
    
    print("🚀 Détection démarrée. Appuyez sur 'q' pour quitter.")
    
        
        # Ouvrir la caméra
        cap = cv2.VideoCapture(0)
        
        if not cap.isOpened():
            print("❌ Erreur: Impossible d'ouvrir la caméra")
            return
        
        # Configuration de la caméra
        cap.set(cv2.CAP_PROP_FRAME_WIDTH, 640)
        cap.set(cv2.CAP_PROP_FRAME_HEIGHT, 480)
        cap.set(cv2.CAP_PROP_FPS, 30)
        
        print("🚀 Détection démarrée. Appuyez sur 'q' pour quitter.")
        print("🎯 Couleurs détectées:")
        for color, config in self.colors.items():
            print(f"  • {config['label']} ({color})")
        
        frame_count = 0
        start_time = time.time()
        
        try:
            while True:
                ret, frame = cap.read()
                if not ret:
                    print("❌ Erreur lecture frame")
                    break
                
                # Conversion en HSV pour la détection de couleur
                hsv = cv2.cvtColor(frame, cv2.COLOR_BGR2HSV)
                
                # Détection et affichage des couleurs
                for color_name, config in self.colors.items():
                    detected = self.draw_color_contours(hsv, frame, color_name, config)
                    
                    if detected:
                        self.capture_and_send_detection(frame, color_name, config)
                
                # Affichage des informations sur le frame
                frame_count += 1
                if frame_count % 30 == 0:  # Chaque seconde environ
                    elapsed = time.time() - start_time
                    fps = frame_count / elapsed
                    
                # Overlay d'informations
                cv2.putText(frame, f"FPS: {fps:.1f}", (10, 30), 
                          cv2.FONT_HERSHEY_SIMPLEX, 0.7, (255, 255, 255), 2)
                cv2.putText(frame, "Appuyez sur 'q' pour quitter", (10, frame.shape[0] - 10), 
                          cv2.FONT_HERSHEY_SIMPLEX, 0.5, (255, 255, 255), 1)
                
                # Affichage de la fenêtre
                if display:
                    cv2.imshow("Detection Couleurs - Système Intégré", frame)
                
                # Gestion des événements clavier
                key = cv2.waitKey(1) & 0xFF
                if key == ord('q'):
                    print("🛑 Arrêt demandé par l'utilisateur")
                    break
                elif key == ord('r'):
                    print("🔄 Réinitialisation des compteurs")
                    self.last_detections = {color: 0 for color in self.colors.keys()}
                
        except KeyboardInterrupt:
            print("\n🛑 Interruption clavier détectée")
            
        finally:
            # Nettoyage
            cap.release()
            if display:
                cv2.destroyAllWindows()
            print("🏁 Détection terminée")
    
    def test_api_connection(self):
        """Tester la connexion à l'API"""
        try:
            response = requests.get(f"{self.api_url}/types", timeout=5)
            if response.status_code == 200:
                types = response.json()
                print(f"✅ API connectée. Types disponibles: {len(types)}")
                for t in types:
                    print(f"  • {t['g_id']}: {t['type_name']}")
                return True
            else:
                print(f"❌ API répond avec erreur: {response.status_code}")
                return False
                
        except requests.exceptions.RequestException as e:
            print(f"🔌 API non accessible: {e}")
            print("💡 Assurez-vous que le serveur Rust est démarré sur le port 3000")
            return False

def main():
    parser = argparse.ArgumentParser(description="Système de détection d'objets colorés")
    parser.add_argument("--camera", type=int, default=0, help="Index de la caméra (défaut: 0)")
    parser.add_argument("--api-url", default="http://127.0.0.1:3000/api", help="URL de l'API")
    parser.add_argument("--no-display", action="store_true", help="Mode headless (sans affichage)")
    parser.add_argument("--test-api", action="store_true", help="Tester la connexion API uniquement")
    
    args = parser.parse_args()
    
    print("🎯 Système de Détection d'Objets Colorés")
    print("=" * 50)
    
    # Initialiser le système
    detector = ColorDetectionSystem(api_url=args.api_url)
    
    # Test de l'API si demandé
    if args.test_api:
        detector.test_api_connection()
        return
    
    # Vérifier la connexion API avant de commencer
    if not detector.test_api_connection():
        print("\n⚠️ Continuons sans connexion API (mode standalone)")
        detector.api_url = None
    
    # Lancer la détection
    detector.run_detection(
        camera_index=args.camera,
        display=not args.no_display
    )

if __name__ == "__main__":
    main()