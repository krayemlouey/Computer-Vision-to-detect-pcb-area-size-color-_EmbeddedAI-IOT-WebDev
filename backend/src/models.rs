use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// Structures de base pour les détections
#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Detection {
    pub id: i64,
    pub g_id: String,
    pub type_name: String,
    pub color: String,
    pub date_time: Option<String>,
    pub image_path: String,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub centroid_x: Option<f64>,
    #[serde(default)]
    pub centroid_y: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct DetectionType {
    pub g_id: String,
    pub type_name: String,
    pub color: Option<String>,
}

// Structures pour les requêtes API
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub success: bool,
    pub message: String,
    pub token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DetectionRequest {
    pub g_id: String,
    pub type_name: String,
    pub color: String,
    pub image_data: String,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub centroid: Option<Vec<f64>>, // [x, y]
    #[serde(default)]
    pub source: Option<String>, // "ESP32", "WebUI", "Flask"
}

#[derive(Debug, Serialize)]
pub struct DetectionResponse {
    pub id: i64,
    pub g_id: String,
    pub type_name: String,
    pub color: String,
    pub date_time: Option<String>,
    pub image_path: String,
    #[serde(default)]
    pub confidence: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct HistoryResponse {
    pub detections: Vec<Detection>,
    pub total: i64,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub counters: Option<CountersResponse>,
}

#[derive(Debug, Deserialize)]
pub struct ExportRequest {
    pub format: Option<String>,
    #[allow(dead_code)]
    pub start_date: Option<String>,
    #[allow(dead_code)]
    pub end_date: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TypeInfo {
    pub g_id: String,
    pub type_name: String,
    pub color: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddTypeRequest {
    pub g_id: String,
    pub type_name: String,
    pub color: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CadenceUpdate {
    pub g_id: String,
    pub cadence: i32,
}

// Structures additionnelles pour l'export et les statistiques
#[derive(Debug, Serialize)]
pub struct ExportResponse {
    pub success: bool,
    pub filename: String,
    pub count: usize,
    pub format: String,
}

#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub total_detections: i64,
    pub recent_24h: i64,
    pub today: i64,
    pub by_type: Vec<TypeStats>,
    pub by_color: Vec<ColorStats>,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct TypeStats {
    pub type_name: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct ColorStats {
    pub color: String,
    pub count: i64,
}

// Structures pour les compteurs temps réel
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CountersResponse {
    pub rouge: i64,
    pub vert: i64,
    pub bleu: i64,
    pub jaune: i64,
    pub noir: i64,
    pub total: i64,
    pub last_update: String,
    pub timestamp: String,
}

#[derive(Debug, Deserialize)]
pub struct ResetCountersRequest {
    pub confirm: bool,
}

// Structure pour les requêtes de nettoyage
#[derive(Debug, Deserialize)]
pub struct CleanupRequest {
    pub days_to_keep: i32,
    pub confirm: bool,
}

// Structure pour les requêtes de sauvegarde
#[derive(Debug, Deserialize)]
pub struct BackupRequest {
    pub name: Option<String>,
}

// === STRUCTURES ESP32 ===

// Requête de mise à jour ESP32
#[derive(Debug, Deserialize)]
pub struct ESP32UpdateRequest {
    pub color: String,      // vert, rouge, bleu, jaune, noir
    pub g_id: String,       // 4 chiffres (ex: 1006)
    pub type_name: String,  // nom composé de A-D et 0-9
    #[serde(default = "default_source")]
    pub source: String,     // "ESP32"
}

fn default_source() -> String {
    "ESP32".to_string()
}

// Réponse de mise à jour ESP32
#[derive(Debug, Serialize)]
pub struct ESP32UpdateResponse {
    pub success: bool,
    pub message: String,
    pub updated_color: Option<String>,
    pub old_mapping: Option<ColorMapping>,
    pub new_mapping: Option<ColorMapping>,
}

// Mapping couleur -> type
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ColorMapping {
    pub color: String,
    pub g_id: String,
    pub type_name: String,
}

// Requête de validation ESP32
#[derive(Debug, Deserialize)]
pub struct ESP32ValidationRequest {
    pub color: String,
    pub g_id: String,
    pub type_name: String,
    #[serde(default = "default_source")]
    pub source: String,
}

// Réponse de validation ESP32
#[derive(Debug, Serialize)]
pub struct ESP32ValidationResponse {
    pub valid: bool,
    pub errors: Vec<String>,
    pub g_id: String,
    pub color: String,
    pub type_name: String,
}

// Appareil ESP32 connecté
#[derive(Debug, Serialize)]
pub struct ESP32Device {
    pub ip: String,
    pub mac_address: Option<String>,
    pub status: String,
    pub last_seen: String,
    pub current_state: Option<String>,
    pub firmware_version: Option<String>,
}

// Liste des appareils ESP32
#[derive(Debug, Serialize)]
pub struct ESP32DevicesResponse {
    pub devices: Vec<ESP32Device>,
    pub count: usize,
    pub last_update: String,
}

// Réponse des mappings
#[derive(Debug, Serialize)]
pub struct MappingsResponse {
    pub success: bool,
    pub mappings: std::collections::HashMap<String, serde_json::Value>,
    pub count: usize,
    pub timestamp: String,
}

// Notification Flask
#[derive(Debug, Deserialize)]
pub struct FlaskNotificationRequest {
    pub updated_color: String,
    pub timestamp: String,
    pub source: String,
}

// Structures étendues pour le système de notification
#[derive(Debug, Serialize)]
pub struct SystemNotification {
    pub type_notification: String, // "mapping_updated", "esp32_connected", etc.
    pub message: String,
    pub data: Option<serde_json::Value>,
    pub timestamp: String,
    pub source: String, // "ESP32", "WebUI", "Flask"
}

// État du système
#[derive(Debug, Serialize)]
pub struct SystemStatus {
    pub status: String,
    pub version: String,
    pub uptime: u64,
    pub database: serde_json::Value,
    pub endpoints: serde_json::Value,
    pub esp32_support: bool,
    pub timestamp: String,
}

// === STRUCTURES SPÉCIALISÉES ESP32 ===

// État du keypad ESP32
#[derive(Debug, Serialize, Deserialize)]
pub struct ESP32KeypadState {
    pub current_input: String,
    pub selected_color: Option<String>,
    pub entered_gid: Option<String>,
    pub entered_name: Option<String>,
    pub state: ESP32InputState,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ESP32InputState {
    Idle,
    SelectingColor,
    EnteringGID,
    EnteringName,
    Confirming,
    Sending,
}

// Configuration ESP32
#[derive(Debug, Serialize, Deserialize)]
pub struct ESP32Config {
    pub wifi_ssid: String,
    pub server_ip: String,
    pub server_port: u16,
    pub lcd_address: u8,
    pub keypad_rows: Vec<u8>,
    pub keypad_cols: Vec<u8>,
    pub supported_colors: Vec<String>,
}

// Réponse de ping ESP32
#[derive(Debug, Serialize)]
pub struct ESP32PingResponse {
    pub status: String,
    pub message: String,
    pub timestamp: String,
    pub server: String,
    pub features: ESP32Features,
    pub endpoints: Vec<String>,
    pub keypad_mapping: ESP32KeypadMapping,
}

#[derive(Debug, Serialize)]
pub struct ESP32Features {
    pub keypad: String,
    pub lcd: String,
    pub colors: u8,
    pub websocket: bool,
}

#[derive(Debug, Serialize)]
pub struct ESP32KeypadMapping {
    pub colors: String,
    pub actions: String,
    pub input: String,
}

// Statistiques ESP32
#[derive(Debug, Serialize)]
pub struct ESP32Stats {
    pub total_updates: i64,
    pub successful_updates: i64,
    pub failed_updates: i64,
    pub last_update: Option<String>,
    pub color_distribution: std::collections::HashMap<String, i64>,
    pub avg_response_time: Option<f64>,
}

// Historique des actions ESP32
#[derive(Debug, Serialize, Deserialize)]
pub struct ESP32Action {
    pub id: i64,
    pub action_type: String, // "color_mapping", "validation", "ping"
    pub color: Option<String>,
    pub g_id: Option<String>,
    pub type_name: Option<String>,
    pub success: bool,
    pub error_message: Option<String>,
    pub response_time_ms: Option<i32>,
    pub timestamp: String,
}

// Validation des données ESP32
#[derive(Debug, Serialize)]
pub struct ESP32ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub suggestions: Vec<String>,
}

// Configuration réseau ESP32
#[derive(Debug, Serialize)]
pub struct ESP32NetworkInfo {
    pub local_ip: String,
    pub port: u16,
    pub esp32_url: String,
    pub websocket_url: String,
    pub endpoints: std::collections::HashMap<String, String>,
}

// === NOTIFICATIONS WEBSOCKET ===

#[derive(Debug, Serialize, Clone)]
pub struct WebSocketNotification {
    pub notification_type: String, // "new_detection", "mapping_update", "counter_update", etc.
    pub data: serde_json::Value,
    pub timestamp: String,
    pub source: String,
}

#[derive(Debug, Serialize)]
pub struct DetectionNotification {
    pub detection: Detection,
    pub counters: CountersResponse,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct CounterUpdateNotification {
    pub counters: CountersResponse,
    pub updated_color: Option<String>,
    pub message: String,
}

// === UTILITAIRES ===

impl DetectionRequest {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.g_id.is_empty() {
            errors.push("G_ID ne peut pas être vide".to_string());
        }

        if self.type_name.is_empty() {
            errors.push("Type name ne peut pas être vide".to_string());
        }

        if self.color.is_empty() {
            errors.push("Couleur ne peut pas être vide".to_string());
        }

        let valid_colors = vec!["rouge", "vert", "bleu", "jaune", "noir"];
        if !valid_colors.contains(&self.color.to_lowercase().as_str()) {
            errors.push(format!("Couleur '{}' non supportée", self.color));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn normalize_color(&mut self) {
        self.color = match self.color.to_lowercase().as_str() {
            "red" => "rouge".to_string(),
            "green" => "vert".to_string(),
            "blue" => "bleu".to_string(),
            "yellow" => "jaune".to_string(),
            "black" => "noir".to_string(),
            _ => self.color.to_lowercase(),
        };
    }
}

impl ESP32UpdateRequest {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // Vérifier G_ID (4 chiffres)
        if self.g_id.len() != 4 || !self.g_id.chars().all(|c| c.is_ascii_digit()) {
            errors.push("G_ID doit contenir exactement 4 chiffres".to_string());
        }

        // Vérifier couleur
        let valid_colors = vec!["vert", "rouge", "bleu", "jaune", "noir"];
        if !valid_colors.contains(&self.color.to_lowercase().as_str()) {
            errors.push(format!("Couleur '{}' non supportée", self.color));
        }

        // Vérifier nom (A-D et 0-9 seulement)
        if self.type_name.is_empty() {
            errors.push("Le nom ne peut pas être vide".to_string());
        } else if !self.type_name.chars().all(|c| {
            c.is_ascii_digit() || matches!(c.to_ascii_uppercase(), 'A'..='D')
        }) {
            errors.push("Nom doit contenir uniquement A-D et 0-9".to_string());
        }

        // Vérifier longueur du nom (max 8 caractères)
        if self.type_name.len() > 8 {
            errors.push("Nom trop long (max 8 caractères)".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl ColorMapping {
    pub fn new(color: String, g_id: String, type_name: String) -> Self {
        Self {
            color,
            g_id,
            type_name,
        }
    }

    pub fn is_valid(&self) -> bool {
        !self.color.is_empty() && 
        !self.g_id.is_empty() && 
        !self.type_name.is_empty() &&
        self.g_id.len() == 4 &&
        self.g_id.chars().all(|c| c.is_ascii_digit())
    }
}

impl Detection {
    pub fn to_websocket_notification(&self) -> WebSocketNotification {
        WebSocketNotification {
            notification_type: "new_detection".to_string(),
            data: serde_json::json!({
                "id": self.id,
                "g_id": self.g_id,
                "type_name": self.type_name,
                "color": self.color,
                "date_time": self.date_time,
                "image_path": self.image_path,
                "confidence": self.confidence
            }),
            timestamp: chrono::Utc::now().to_rfc3339(),
            source: "Database".to_string(),
        }
    }
}

impl CountersResponse {
    pub fn to_websocket_notification(&self, updated_color: Option<String>) -> WebSocketNotification {
        WebSocketNotification {
            notification_type: "counter_update".to_string(),
            data: serde_json::json!({
                "counters": {
                    "rouge": self.rouge,
                    "vert": self.vert,
                    "bleu": self.bleu,
                    "jaune": self.jaune,
                    "noir": self.noir,
                    "total": self.total
                },
                "updated_color": updated_color,
                "last_update": self.last_update
            }),
            timestamp: chrono::Utc::now().to_rfc3339(),
            source: "Counter".to_string(),
        }
    }
}

impl ESP32Config {
    pub fn default() -> Self {
        Self {
            wifi_ssid: "WIFI_SSID".to_string(),
            server_ip: "192.168.1.100".to_string(),
            server_port: 3001,
            lcd_address: 0x27,
            keypad_rows: vec![19, 18, 5, 17],
            keypad_cols: vec![16, 4, 2, 15],
            supported_colors: vec![
                "vert".to_string(),
                "rouge".to_string(),
                "bleu".to_string(),
                "jaune".to_string(),
                "noir".to_string(),
            ],
        }
    }
}

// Utilitaires pour normaliser les couleurs
pub fn normalize_color_name(color: &str) -> String {
    match color.to_lowercase().trim() {
        "red" | "rouge" => "rouge".to_string(),
        "green" | "vert" => "vert".to_string(),
        "blue" | "bleu" => "bleu".to_string(),
        "yellow" | "jaune" => "jaune".to_string(),
        "black" | "noir" => "noir".to_string(),
        _ => color.to_lowercase(),
    }
}

pub fn capitalize_color(color: &str) -> String {
    match color.to_lowercase().trim() {
        "rouge" => "Rouge".to_string(),
        "vert" => "Vert".to_string(),
        "bleu" => "Bleu".to_string(),
        "jaune" => "Jaune".to_string(),
        "noir" => "Noir".to_string(),
        _ => {
            let mut chars = color.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        }
    }
}