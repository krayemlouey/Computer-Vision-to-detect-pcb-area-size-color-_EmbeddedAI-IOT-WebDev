#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Service de détection de couleurs - Module principal
Compatible avec Flask et fonctionnement standalone
"""

import cv2
import numpy as np
import threading
import time
from datetime import datetime
import json

class DetectionService:
    def __init__(self, camera_index=0, socketio=None):
        self.camera_index = camera_index
        self.cap = None
        self.camera_active = False
        self.frame_count = 0
        self.current_frame = None
        self.detection_worker = None
        self.stop_detection = False
        self.frame_lock = threading.Lock()
        self.socketio = socketio  # Référence au socketio pour émettre les événements
        
        # Statistiques
        self.detection_count = 0
        self.last_detections = {}
        self.detection_cooldown = 2.0  # 2 secondes entre détections de même couleur
        
        # Cache des résultats de détection pour éliminer la latence
        self.cached_validated_contours = {}
        self.cached_tracked_centroids = {}
        self.cache_lock = threading.Lock()
        
        # Configuration des couleurs HSV optimisées (renforcées pour faible luminosité et ombres)
        self.colors = {
            'Rouge': {
                'hsv_ranges': [
                    (np.array([0, 100, 70]), np.array([8, 255, 255])),
                    (np.array([172, 100, 70]), np.array([180, 255, 255]))
                ],
                'contour_color': (0, 0, 255),
                'min_area': 1500
            },
            'Vert': {
                'hsv_ranges': [
                    (np.array([35, 40, 40]), np.array([89, 255, 255]))
                ],
                'contour_color': (0, 255, 0),
                'min_area': 1500
            },
            'Bleu': {
                'hsv_ranges': [
                    (np.array([90, 40, 40]), np.array([130, 255, 255]))
                ],
                'contour_color': (255, 0, 0),
                'min_area': 1500
            },
            'Noir': {
                'hsv_ranges': [
                    (np.array([0, 0, 0]), np.array([180, 255, 55]))
                ],
                'contour_color': (128, 128, 128),
                'min_area': 1500
            }
        }
        
        self.active_colors = list(self.colors.keys())
        # Kernel pour morphologie
        self.morphology_kernel = cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (5, 5))
        
        print("Service de détection initialisé avec cache anti-latence")

    def set_active_colors(self, colors):
        """Met à jour la liste des couleurs actives à détecter."""
        self.active_colors = [c.capitalize() for c in colors]
        print(f"Couleurs OpenCV actives: {self.active_colors}")

    def start_camera(self):
        """Démarre la caméra et le worker de détection"""
        try:
            if self.camera_active:
                print("Caméra déjà active")
                return True
            
            # Initialiser la capture vidéo
            self.cap = cv2.VideoCapture(self.camera_index)
            
            if not self.cap.isOpened():
                print(f"Impossible d'ouvrir la caméra {self.camera_index}")
                return False
            
            # Configuration optimisée
            self.cap.set(cv2.CAP_PROP_FRAME_WIDTH, 1280)
            self.cap.set(cv2.CAP_PROP_FRAME_HEIGHT, 720)
            self.cap.set(cv2.CAP_PROP_FPS, 30)
            self.cap.set(cv2.CAP_PROP_BUFFERSIZE, 1)
            
            # Test de lecture
            ret, test_frame = self.cap.read()
            if not ret:
                print("Impossible de lire depuis la caméra")
                self.cap.release()
                return False
            
            self.camera_active = True
            self.stop_detection = False
            self.frame_count = 0
            self.detection_count = 0
            self.last_detections = {}
            
            # Démarrer le worker de détection
            self.detection_worker = threading.Thread(target=self._detection_worker, daemon=True)
            self.detection_worker.start()
            
            print("Worker de détection démarré")
            return True
            
        except Exception as e:
            print(f"Erreur démarrage caméra: {e}")
            if self.cap:
                self.cap.release()
            return False

    def stop_camera(self):
        """Arrête la caméra et le worker"""
        try:
            print("Arrêt de la caméra...")
            self.stop_detection = True
            self.camera_active = False
            
            # Attendre l'arrêt du worker
            if self.detection_worker and self.detection_worker.is_alive():
                self.detection_worker.join(timeout=2.0)
                print("Worker de détection arrêté")
            
            # Libérer la caméra
            if self.cap:
                self.cap.release()
                self.cap = None
            
            with self.frame_lock:
                self.current_frame = None
            
            return True
            
        except Exception as e:
            print(f"Erreur arrêt caméra: {e}")
            return False

    def _detection_worker(self):
        """Worker principal pour la capture continue en temps réel sans latence de buffering"""
        print("Worker de capture/détection démarré")
        
        last_process_time = 0
        process_interval = 0.1  # Traiter la détection 10 fois par seconde (toutes les 100ms)
        
        while not self.stop_detection and self.camera_active:
            try:
                if not self.cap or not self.cap.isOpened():
                    break
                
                # Lire continuellement la caméra pour purger le buffer OpenCV
                ret, frame = self.cap.read()
                if not ret or frame is None:
                    time.sleep(0.01)
                    continue
                
                self.frame_count += 1
                
                # Sauvegarder la frame courante de manière thread-safe
                with self.frame_lock:
                    self.current_frame = frame.copy()
                
                # Traiter la détection de manière asynchrone à intervalle régulier (10 FPS)
                current_time = time.time()
                if current_time - last_process_time >= process_interval:
                    self._process_detections(frame)
                    last_process_time = current_time
                
            except Exception as e:
                print(f"Erreur dans worker détection: {e}")
                time.sleep(0.05)
        
        print("Worker de détection arrêté")

    def _process_detections(self, frame):
        """Traite les détections et met à jour le cache thread-safe"""
        try:
            validated_contours, tracked_centroids = self.process_frame(frame)
            
            # Enregistrer les résultats de détection dans le cache
            with self.cache_lock:
                self.cached_validated_contours = validated_contours
                self.cached_tracked_centroids = tracked_centroids
            
            current_time = time.time()
            
            for color_name, (contour, (cx, cy)) in validated_contours.items():
                # Vérifier le cooldown
                last_detection = self.last_detections.get(color_name, 0)
                if current_time - last_detection < self.detection_cooldown:
                    continue
                
                # Nouvelle détection valide
                area = cv2.contourArea(contour)
                self.detection_count += 1
                self.last_detections[color_name] = current_time
                
                # Données de détection
                detection_data = {
                    'color': color_name,
                    'centroid': [int(cx), int(cy)],
                    'area': int(area),
                    'timestamp': datetime.now().isoformat(),
                    'frame_count': self.frame_count
                }
                
                print(f"Détection validée: {color_name} à ({cx}, {cy})")
                
                # Émettre l'événement WebSocket si socketio est disponible
                if self.socketio:
                    self.socketio.emit('detection_update', detection_data)
                
        except Exception as e:
            print(f"Erreur traitement détections: {e}")

    def get_current_frame(self):
        """Récupère la frame courante de manière thread-safe"""
        with self.frame_lock:
            return self.current_frame.copy() if self.current_frame is not None else None

    def process_frame(self, frame):
        """Traite une frame avec sous-échantillonnage pour de la détection ultra-rapide (Zéro latence)"""
        if frame is None:
            return {}, {}
        
        try:
            h, w = frame.shape[:2]
            proc_w = 320
            proc_h = int(h * (proc_w / w))
            scale_x = w / proc_w
            scale_y = h / proc_h
            
            # Prétraitement sur l'image réduite pour multiplier la vitesse par 16
            small_frame = cv2.resize(frame, (proc_w, proc_h))
            blurred = cv2.GaussianBlur(small_frame, (9, 9), 0)
            hsv = cv2.cvtColor(blurred, cv2.COLOR_BGR2HSV)
            
            validated_contours = {}
            tracked_centroids = {}
            
            # Seuil d'aire équilibré pour permettre la détection sans capter trop de bruit
            min_area_proc = 2500
            
            for color_name, config in self.colors.items():
                if color_name not in self.active_colors:
                    continue
                    
                # Créer le masque couleur
                mask = self._create_color_mask(hsv, config)
                
                # Trouver les contours
                contours, _ = cv2.findContours(mask, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_SIMPLE)
                
                # Filtrer les contours valides avec la contrainte géométrique et de proximité
                valid_contours = []
                for c in contours:
                    area = cv2.contourArea(c)
                    if area >= min_area_proc:
                        if self._is_valid_card_shape(c, proc_w, proc_h):
                            valid_contours.append((c, area))
                
                if valid_contours:
                    # Prendre le plus grand contour valide
                    largest_contour, area = max(valid_contours, key=lambda x: x[1])
                    
                    # Calculer le centroïde
                    M = cv2.moments(largest_contour)
                    if M['m00'] != 0:
                        cx = int(M['m10'] / M['m00'])
                        cy = int(M['m01'] / M['m00'])
                        
                        # Remettre à l'échelle pour l'image haute résolution d'origine
                        original_contour = largest_contour.copy()
                        original_contour[:, :, 0] = np.round(original_contour[:, :, 0] * scale_x).astype(int)
                        original_contour[:, :, 1] = np.round(original_contour[:, :, 1] * scale_y).astype(int)
                        
                        orig_cx = int(cx * scale_x)
                        orig_cy = int(cy * scale_y)
                        
                        validated_contours[color_name] = (original_contour, (orig_cx, orig_cy))
                        tracked_centroids[color_name] = (orig_cx, orig_cy)
            # Priorité : si une autre couleur est détectée, le Noir est ignoré
            if 'Noir' in validated_contours and len(validated_contours) > 1:
                del validated_contours['Noir']
                if 'Noir' in tracked_centroids:
                    del tracked_centroids['Noir']
            
            return validated_contours, tracked_centroids
            
        except Exception as e:
            print(f"Erreur traitement frame: {e}")
            return {}, {}

    def _is_valid_card_shape(self, contour, proc_w=320, proc_h=180):
        """Vérifie si la forme géométrique du contour correspond à une carte électronique rectangulaire (avec tolérance)"""
        try:
            # 1. Aire et enveloppe convexe
            area = cv2.contourArea(contour)
            
            # Ne doit pas être un arrière-plan géant (comme un mur)
            # Une carte approchée de la caméra peut occuper jusqu'à 60% de l'image
            max_area_proc = int(proc_w * proc_h * 0.60)
            if area > max_area_proc:
                return False
                
            hull = cv2.convexHull(contour)
            hull_area = cv2.contourArea(hull)
            
            if hull_area == 0:
                return False
                
            # Solidité (proche de 1.0 pour un rectangle, mais tolérant aux doigts tenant la carte et aux composants)
            solidity = float(area) / hull_area
            
            # Rejeter les formes organiques complexes (doigts, visage, vêtements ont une solidité faible)
            if solidity < 0.70:
                return False
                
            # 2. Rectangle englobant
            x, y, w, h = cv2.boundingRect(contour)
            if w == 0 or h == 0:
                return False
                
            # Vérifier si l'objet touche trop de bords de l'image (indice d'un mur d'arrière-plan ou d'une table)
            touches_left = x <= 2
            touches_right = (x + w) >= (proc_w - 2)
            touches_top = y <= 2
            touches_bottom = (y + h) >= (proc_h - 2)
            
            borders_touched = sum([touches_left, touches_right, touches_top, touches_bottom])
            if borders_touched >= 3:
                # Touche 3 bords ou plus : c'est un arrière-plan (mur, table, etc.)
                return False
                
            # Aspect ratio de la carte (doit être raisonnable pour un rectangle de circuit)
            aspect_ratio = float(w) / h
            if aspect_ratio < 0.35 or aspect_ratio > 2.8:
                return False
                
            # Rectangularité / Étendue (Extent)
            # Permet des composants manquants ou de couleur différente sur les côtés de la carte
            extent = float(area) / (w * h)
            if extent < 0.45:
                return False
                
            return True
        except:
            return False

    def get_cached_detections(self):
        """Retourne les contours et centroïdes validés stockés en cache de manière thread-safe"""
        with self.cache_lock:
            return self.cached_validated_contours.copy(), self.cached_tracked_centroids.copy()

    def _create_color_mask(self, hsv_img, color_config):
        """Crée un masque pour une couleur donnée (version ultra-rapide)"""
        mask = np.zeros(hsv_img.shape[:2], dtype=np.uint8)
        
        # Combiner toutes les plages HSV
        for hsv_range in color_config['hsv_ranges']:
            range_mask = cv2.inRange(hsv_img, hsv_range[0], hsv_range[1])
            mask = cv2.bitwise_or(mask, range_mask)
        
        # Opérations morphologiques pour éliminer le bruit de lumière
        mask = cv2.morphologyEx(mask, cv2.MORPH_OPEN, self.morphology_kernel)
        mask = cv2.morphologyEx(mask, cv2.MORPH_CLOSE, self.morphology_kernel)
        mask = cv2.dilate(mask, self.morphology_kernel, iterations=1)
        
        return mask

    def generate_frames(self):
        """Générateur de frames ultra-fluide pour le streaming Flask"""
        while True:
            try:
                frame = self.get_current_frame()
                
                if frame is None:
                    # Frame par défaut si pas de caméra
                    frame = np.zeros((480, 640, 3), dtype=np.uint8)
                    cv2.putText(frame, "Camera Inactive", (50, 240), 
                              cv2.FONT_HERSHEY_SIMPLEX, 2, (255, 255, 255), 3)
                else:
                    # Dessiner les détections pré-calculées depuis le cache (0 calcul redondant)
                    frame = self._draw_detections_on_frame(frame)
                
                # Encodage JPEG
                ret, buffer = cv2.imencode('.jpg', frame, 
                    [cv2.IMWRITE_JPEG_QUALITY, 80])
                
                if not ret:
                    continue
                
                frame_bytes = buffer.tobytes()
                
                yield (b'--frame\r\n'
                       b'Content-Type: image/jpeg\r\n\r\n' + frame_bytes + b'\r\n')
                
                time.sleep(0.033)  # ~30 FPS
                
            except Exception as e:
                print(f"Erreur génération frames: {e}")
                time.sleep(0.1)

    def _draw_detections_on_frame(self, frame):
        """Dessine les détections pré-calculées en arrière-plan sans recalculer l'image"""
        try:
            # Récupérer les contours du cache
            validated_contours, _ = self.get_cached_detections()
            
            # Dessiner les contours et labels
            for color_name, (contour, (cx, cy)) in validated_contours.items():
                config = self.colors[color_name]
                
                # Dessiner le contour
                cv2.drawContours(frame, [contour], -1, config['contour_color'], 3)
                
                # Rectangle englobant
                x, y, w, h = cv2.boundingRect(contour)
                cv2.rectangle(frame, (x, y), (x + w, y + h), config['contour_color'], 2)
                
                # Label avec fond
                label = f"{color_name} (Proche)"
                label_size = cv2.getTextSize(label, cv2.FONT_HERSHEY_SIMPLEX, 0.7, 2)[0]
                cv2.rectangle(frame, (cx - label_size[0]//2 - 5, cy - label_size[1] - 10),
                            (cx + label_size[0]//2 + 5, cy + 5), config['contour_color'], -1)
                cv2.putText(frame, label, (cx - label_size[0]//2, cy - 5), 
                          cv2.FONT_HERSHEY_SIMPLEX, 0.7, (255, 255, 255), 2)
                
                # Aire du contour originale (à l'échelle de l'image complète)
                area = cv2.contourArea(contour)
                info = f"Aire: {int(area)}"
                cv2.putText(frame, info, (x, y - 10), 
                          cv2.FONT_HERSHEY_SIMPLEX, 0.5, config['contour_color'], 1)
            
            # Informations générales
            info_text = f"Frame: {self.frame_count} | Detections: {self.detection_count}"
            cv2.putText(frame, info_text, (10, 30), 
                      cv2.FONT_HERSHEY_SIMPLEX, 0.7, (0, 255, 0), 2)
            
            return frame
            
        except Exception as e:
            print(f"Erreur dessin détections: {e}")
            return frame

    def get_status(self):
        """Retourne le statut du service"""
        return {
            "camera_active": self.camera_active,
            "frame_count": self.frame_count,
            "detection_count": self.detection_count,
            "camera_index": self.camera_index,
            "colors_configured": len(self.colors),
            "timestamp": datetime.now().isoformat()
        }

    def test_camera(self):
        """Test rapide de la caméra"""
        try:
            test_cap = cv2.VideoCapture(self.camera_index)
            if test_cap.isOpened():
                ret, frame = test_cap.read()
                test_cap.release()
                return ret and frame is not None
            return False
        except Exception as e:
            print(f"Erreur test caméra: {e}")
            return False

# Test du module si lancé directement
if __name__ == "__main__":
    print("Test du service de détection")
    
    service = DetectionService()
    
    if service.test_camera():
        print("✅ Caméra accessible")
        
        if service.start_camera():
            print("✅ Service démarré")
            
            try:
                time.sleep(5)  # Test pendant 5 secondes
                status = service.get_status()
                print(f"Status: {status}")
                
            except KeyboardInterrupt:
                print("Interruption utilisateur")
            finally:
                service.stop_camera()
                print("✅ Service arrêté")
        else:
            print("❌ Impossible de démarrer le service")
    else:
        print("❌ Caméra non accessible")