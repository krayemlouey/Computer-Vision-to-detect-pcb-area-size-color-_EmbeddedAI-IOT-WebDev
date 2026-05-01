#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Service de Détection de Couleurs - Version Corrigée
Amélioration de la gestion caméra et détection en temps réel
"""

import cv2
import numpy as np
import time
import threading
from collections import deque
from datetime import datetime
import logging
import os

# Configuration des logs
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

class DetectionService:
    def __init__(self):
        self.cap = None
        self.frame_count = 0
        self.current_frame = None
        self.camera_active = False
        self.frame_lock = threading.Lock()
        
        # Configuration des couleurs HSV
        self.color_ranges = {
            'Rouge': {
                'lower1': np.array([0, 120, 70]),
                'upper1': np.array([10, 255, 255]),
                'lower2': np.array([170, 120, 70]),
                'upper2': np.array([180, 255, 255])
            },
            'Vert': {
                'lower': np.array([35, 100, 100]),
                'upper': np.array([80, 255, 255])
            },
            'Bleu': {
                'lower': np.array([100, 150, 0]),
                'upper': np.array([140, 255, 255])
            },
            'Jaune': {
                'lower': np.array([20, 100, 100]),
                'upper': np.array([30, 255, 255])
            },
            'Noir': {
                'lower': np.array([0, 0, 0]),
                'upper': np.array([180, 255, 30])
            }
        }
        
        # Paramètres de détection
        self.min_contour_area = 1500
        self.detection_stability_frames = 5
        self.tracked_objects = {}
        
        logger.info("[INFO] Service de détection initialisé")

    def start_camera(self):
        """Démarre la capture vidéo"""
        try:
            if self.camera_active:
                logger.warning("[WARN] Caméra déjà active")
                return True
            
            # Essayer différents indices de caméra
            camera_indices = [0, 1, 2, -1]
            
            for index in camera_indices:
                logger.info(f"[INFO] Tentative de connexion caméra index {index}")
                self.cap = cv2.VideoCapture(index)
                
                if self.cap is not None and self.cap.isOpened():
                    # Configuration de la caméra
                    self.cap.set(cv2.CAP_PROP_FRAME_WIDTH, 640)
                    self.cap.set(cv2.CAP_PROP_FRAME_HEIGHT, 480)
                    self.cap.set(cv2.CAP_PROP_FPS, 30)
                    
                    # Test de lecture d'une frame
                    ret, test_frame = self.cap.read()
                    if ret and test_frame is not None:
                        logger.info(f"[OK] Caméra connectée avec succès (index {index})")
                        self.camera_active = True
                        self.frame_count = 0
                        
                        # Démarrer le thread de capture
                        self._start_capture_thread()
                        return True
                    else:
                        self.cap.release()
                        self.cap = None
                
                logger.warning(f"[WARN] Échec connexion caméra index {index}")
            
            logger.error("[ERROR] Impossible de se connecter à une caméra")
            return False
            
        except Exception as e:
            logger.error(f"[ERROR] Erreur démarrage caméra: {e}")
            return False

    def _start_capture_thread(self):
        """Démarre le thread de capture en arrière-plan"""
        self.capture_thread = threading.Thread(target=self._capture_worker, daemon=True)
        self.capture_thread.start()
        logger.info("[INFO] Thread de capture démarré")

    def _capture_worker(self):
        """Worker thread pour capturer les frames en continu"""
        while self.camera_active and self.cap is not None:
            try:
                ret, frame = self.cap.read()
                if ret and frame is not None:
                    with self.frame_lock:
                        self.current_frame = frame.copy()
                        self.frame_count += 1
                else:
                    logger.warning("[WARN] Frame non capturée")
                    time.sleep(0.1)
                    
            except Exception as e:
                logger.error(f"[ERROR] Erreur dans capture_worker: {e}")
                break
        
        logger.info("[INFO] Thread de capture arrêté")

    def stop_camera(self):
        """Arrête la capture vidéo"""
        try:
            logger.info("[INFO] Arrêt de la caméra...")
            self.camera_active = False
            
            if hasattr(self, 'capture_thread'):
                self.capture_thread.join(timeout=2.0)
            
            if self.cap is not None:
                self.cap.release()
                self.cap = None
            
            with self.frame_lock:
                self.current_frame = None
            
            logger.info("[OK] Caméra arrêtée avec succès")
            
        except Exception as e:
            logger.error(f"[ERROR] Erreur arrêt caméra: {e}")

    def get_current_frame(self):
        """Retourne la frame courante de manière thread-safe"""
        with self.frame_lock:
            if self.current_frame is not None:
                return self.current_frame.copy()
            return None

    def process_frame(self, frame, save_detections=False):
        """Traite une frame pour détecter les couleurs et optionnellement sauvegarder"""
        if frame is None:
            return {}, {}
        
        try:
            # Conversion en HSV pour une meilleure détection des couleurs
            hsv = cv2.cvtColor(frame, cv2.COLOR_BGR2HSV)
            
            # Lissage pour réduire le bruit
            hsv = cv2.GaussianBlur(hsv, (5, 5), 0)
            
            validated_contours = {}
            tracked_centroids = {}
            
            for color_name, ranges in self.color_ranges.items():
                mask = self._create_color_mask(hsv, ranges)
                contours = self._find_contours(mask)
                
                if contours:
                    # Prendre le plus grand contour
                    largest_contour = max(contours, key=cv2.contourArea)
                    area = cv2.contourArea(largest_contour)
                    
                    if area > self.min_contour_area:
                        # Calculer le centroïde
                        M = cv2.moments(largest_contour)
                        if M["m00"] != 0:
                            cx = int(M["m10"] / M["m00"])
                            cy = int(M["m01"] / M["m00"])
                            
                            # Validation de la stabilité
                            if self._validate_detection_stability(color_name, (cx, cy)):
                                validated_contours[color_name] = (largest_contour, (cx, cy))
                                tracked_centroids[color_name] = (cx, cy)
                                
                                # Sauvegarder l'image si demandé
                                if save_detections:
                                    saved_filename = self.save_detection_image(frame, color_name, largest_contour)
                                    if saved_filename:
                                        # Émettre un événement de nouvelle détection
                                        self.emit_detection_event(color_name, (cx, cy), area, saved_filename)
            
            return validated_contours, tracked_centroids
            
        except Exception as e:
            logger.error(f"[ERROR] Erreur traitement frame: {e}")
            return {}, {}

    def emit_detection_event(self, color_name, centroid, area, image_filename):
        """Émet un événement de détection"""
        detection_data = {
            'color': color_name,
            'centroid': centroid,
            'area': int(area),
            'image_filename': image_filename,
            'timestamp': datetime.now().isoformat()
        }
        
        logger.info(f"[EVENT] Détection émise: {color_name} at {centroid}")

    def _create_color_mask(self, hsv, ranges):
        """Crée un masque pour une couleur donnée"""
        if 'lower1' in ranges and 'lower2' in ranges:
            # Pour le rouge qui traverse la limite HSV (0-180)
            mask1 = cv2.inRange(hsv, ranges['lower1'], ranges['upper1'])
            mask2 = cv2.inRange(hsv, ranges['lower2'], ranges['upper2'])
            mask = cv2.bitwise_or(mask1, mask2)
        else:
            # Pour les autres couleurs
            mask = cv2.inRange(hsv, ranges['lower'], ranges['upper'])
        
        # Opérations morphologiques pour nettoyer le masque
        kernel = np.ones((5, 5), np.uint8)
        mask = cv2.morphologyEx(mask, cv2.MORPH_CLOSE, kernel)
        mask = cv2.morphologyEx(mask, cv2.MORPH_OPEN, kernel)
        
        return mask

    def _find_contours(self, mask):
        """Trouve les contours dans un masque"""
        contours, _ = cv2.findContours(mask, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_SIMPLE)
        
        # Filtrer les contours par taille
        valid_contours = [c for c in contours if cv2.contourArea(c) > self.min_contour_area]
        
        return valid_contours

    def _validate_detection_stability(self, color_name, centroid, threshold=50):
        """Valide la stabilité d'une détection"""
        current_time = time.time()
        
        if color_name not in self.tracked_objects:
            self.tracked_objects[color_name] = deque(maxlen=self.detection_stability_frames)
        
        # Ajouter la détection actuelle
        self.tracked_objects[color_name].append({
            'centroid': centroid,
            'timestamp': current_time
        })
        
        # Si on n'a pas assez d'échantillons, accepter
        if len(self.tracked_objects[color_name]) < 2:
            return True
        
        # Vérifier la stabilité par rapport à la détection précédente
        prev_detection = self.tracked_objects[color_name][-2]
        distance = np.sqrt((centroid[0] - prev_detection['centroid'][0])**2 + 
                          (centroid[1] - prev_detection['centroid'][1])**2)
        
        return distance < threshold

    def draw_detections(self, frame, validated_contours):
        """Dessine les détections sur la frame"""
        if frame is None:
            return frame
            
        result_frame = frame.copy()
        
        for color_name, (contour, (cx, cy)) in validated_contours.items():
            # Couleurs pour l'affichage
            color_map = {
                'Rouge': (0, 0, 255),
                'Vert': (0, 255, 0),
                'Bleu': (255, 0, 0),
                'Jaune': (0, 255, 255),
                'Noir': (128, 128, 128)
            }
            
            color_bgr = color_map.get(color_name, (255, 255, 255))
            
            # Dessiner le contour
            cv2.drawContours(result_frame, [contour], -1, color_bgr, 2)
            
            # Dessiner le centroïde
            cv2.circle(result_frame, (cx, cy), 7, color_bgr, -1)
            
            # Ajouter le texte
            cv2.putText(result_frame, f"{color_name} ({cx},{cy})", 
                       (cx + 10, cy - 10), cv2.FONT_HERSHEY_SIMPLEX, 
                       0.5, color_bgr, 2)
            
            # Calculer et afficher l'aire
            area = cv2.contourArea(contour)
            cv2.putText(result_frame, f"Aire: {int(area)}", 
                       (cx + 10, cy + 10), cv2.FONT_HERSHEY_SIMPLEX, 
                       0.4, color_bgr, 1)
        
        return result_frame

    def save_detection_image(self, frame, color_name, contour):
        """Sauvegarde une image de détection"""
        try:
            # Créer le dossier s'il n'existe pas
            captures_dir = os.path.join(os.path.dirname(__file__), "frontend", "assets", "captures")
            os.makedirs(captures_dir, exist_ok=True)
            
            # Générer un nom de fichier unique
            timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
            filename = f"detection_{color_name.lower()}_{timestamp}.jpg"
            filepath = os.path.join(captures_dir, filename)
            
            # Créer une image avec la détection encadrée
            result_frame = frame.copy()
            
            # Couleurs pour l'encadrement
            color_map = {
                'Rouge': (0, 0, 255),
                'Vert': (0, 255, 0),
                'Bleu': (255, 0, 0),
                'Jaune': (0, 255, 255),
                'Noir': (128, 128, 128)
            }
            
            color_bgr = color_map.get(color_name, (255, 255, 255))
            
            # Dessiner le contour et les informations
            cv2.drawContours(result_frame, [contour], -1, color_bgr, 3)
            
            # Ajouter un timestamp sur l'image
            cv2.putText(result_frame, f"{color_name} - {timestamp}", 
                       (10, 30), cv2.FONT_HERSHEY_SIMPLEX, 1, color_bgr, 2)
            
            # Sauvegarder l'image
            success = cv2.imwrite(filepath, result_frame)
            
            if success:
                logger.info(f"[OK] Image sauvegardée: {filename}")
                return filename
            else:
                logger.error(f"[ERROR] Échec sauvegarde image: {filename}")
                return None
                
        except Exception as e:
            logger.error(f"[ERROR] Erreur sauvegarde image: {e}")
            return None

    def __del__(self):
        """Destructeur - s'assure que la caméra est libérée"""
        self.stop_camera()

if __name__ == "__main__":
    service = DetectionService()
    if service.start_camera():
        logger.info("[INFO] Appuyez sur 'q' pour quitter")
        try:
            while True:
                frame = service.get_current_frame()
                if frame is not None:
                    validated_contours, _ = service.process_frame(frame)
                    annotated_frame = service.draw_detections(frame, validated_contours)
                    cv2.imshow("Color Detection Test", annotated_frame)
                    
                    if cv2.waitKey(1) & 0xFF == ord('q'):
                        break
        except KeyboardInterrupt:
            pass
        finally:
            cv2.destroyAllWindows()
            service.stop_camera()