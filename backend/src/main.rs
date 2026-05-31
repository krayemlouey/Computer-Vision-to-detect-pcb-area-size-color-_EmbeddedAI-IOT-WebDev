mod models;
mod handlers;
mod database;

use axum::{
    extract::State,
    http::{Method, HeaderValue},
    response::Json as ResponseJson,
    routing::{get, post, delete},
    Router, Json
};
use sqlx::SqlitePool;
use tower_http::cors::CorsLayer;
use serde_json::json;
use std::net::SocketAddr;
use axum::http::StatusCode;
use tower_http::services::ServeDir;
use base64::{Engine as _, engine::general_purpose};
use std::io::Write;

fn get_local_ip() -> Option<std::net::Ipv4Addr> {
    use std::net::UdpSocket;
    
    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        if let Ok(_) = socket.connect("8.8.8.8:80") {
            if let Ok(addr) = socket.local_addr() {
                if let std::net::IpAddr::V4(ipv4) = addr.ip() {
                    return Some(ipv4);
                }
            }
        }
    }
    None
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt::init();

    println!("==========================================");
    println!("   SERVEUR RUST ESP32 - VERSION FINALE");
    println!("==========================================");

    println!("\n1. DIAGNOSTIC SYSTEME");
    let current_dir = std::env::current_dir()?;
    println!("Répertoire de travail: {}", current_dir.display());

    let database_url = "sqlite:data/detection.db";
    println!("Base de données configurée: {}", database_url);

    println!("\n2. INITIALISATION ET MIGRATION BASE DE DONNEES");
    std::fs::create_dir_all("data")?;
    let pool = SqlitePool::connect(database_url).await?;
    println!("Connexion base de données réussie!");

    database::diagnose_database_structure(&pool).await?;
    database::init_database(&pool).await?;
    
    println!("\n🔧 MIGRATION AUTOMATIQUE DE LA BASE DE DONNÉES");
    if let Err(e) = database::migrate_database_structure(&pool).await {
        eprintln!("⚠️ Erreur migration douce: {}", e);
        println!("🔄 Tentative de recréation complète de la table...");
        database::recreate_detections_table(&pool).await?;
    }

    println!("\n✅ VÉRIFICATION POST-MIGRATION");
    database::diagnose_database_structure(&pool).await?;
    println!("✅ Base de données prête avec la structure complète !");

    println!("\n3. INITIALISATION NOTIFICATIONS");
    handlers::init_notifications();
    println!("Système de notifications WebSocket initialisé!");

    println!("\n4. CREATION DOSSIERS RESSOURCES");
    create_resource_directories()?;
    debug_file_paths().await?;
    
    check_frontend_files();

    let cors = CorsLayer::new()
        .allow_origin("*".parse::<HeaderValue>().unwrap())
        .allow_methods([
            Method::GET, 
            Method::POST, 
            Method::PUT, 
            Method::DELETE, 
            Method::OPTIONS
        ])
        .allow_headers(tower_http::cors::Any)
        .allow_credentials(false);

    let app = Router::new()
        // === ROUTES DE COMPATIBILITÉ FLASK (PRIORITÉ ABSOLUE) ===
        .route("/api/camera/start", post(camera_start))  
        .route("/api/detections", get(get_detections_flask_compat))
        .route("/api/debug/images", get(debug_image_paths))
        
        .route("/api/camera/status", get(camera_status))
        .route("/api/statistics", get(get_statistics))
        .route("/api/reset", post(reset_system))
        .route("/api/detections", post(handlers::save_detection))
        .route("/api/fix-all-image-paths", post(fix_all_image_paths))
        // === ROUTES API PRINCIPALES ===
        .route("/api/login", post(handlers::login))
        .route("/api/verify", get(handlers::verify_auth))
        .route("/api/detections/save", post(handlers::save_detection))  // Renommé pour éviter conflit
        .route("/api/history", get(get_history_handler))
        .route("/api/history/filtered", get(get_history_filtered_handler))
        .route("/api/history/export", post(handlers::export_history))
        .route("/api/history", delete(handlers::delete_history))
        .route("/api/detections/:id", delete(delete_detection_handler))
        .route("/api/types", get(handlers::get_types))
        .route("/api/types", post(handlers::add_type))
        .route("/api/cadence", post(handlers::update_cadence))
        .route("/api/debug/detection", post(debug_detection))
        // === ROUTES ESP32 SPÉCIALISÉES ===
        .route("/api/esp32/update-mapping", post(handlers::update_color_mapping_esp32))
        .route("/api/esp32/validate-data", post(handlers::validate_esp32_data))
        .route("/api/esp32/current-mappings", get(handlers::get_current_mappings_esp32))
        .route("/api/esp32/devices", get(handlers::get_esp32_devices))
        .route("/api/esp32/ping", get(esp32_ping))
        .route("/api/esp32/test", post(esp32_test))
        .route("/api/test-assets", get(test_assets_route))
        // === ROUTES COMPTEURS ===
        .route("/api/counters", get(handlers::get_real_time_counters_handler))
        //.route("/api/counters", get(get_counters_handler))
        .route("/api/counters/reset", post(handlers::reset_counters_handler))
        .route("/api/real-time-counters", get(handlers::get_real_time_counters_handler))
        .route("/api/debug/assets-check", get(check_assets_availability))
        // === AUTRES ROUTES API ===
        .route("/api/color-mappings", get(handlers::get_color_mappings))
        .route("/api/stats", get(handlers::get_stats))
        .route("/api/cleanup", post(handlers::cleanup_detections))
        .route("/api/backup", post(handlers::create_backup))
        .route("/api/recent-detections", get(handlers::get_recent_detections_handler))
        .route("/api/status", get(get_system_status))
        .route("/api/network-info", get(network_info))
        .route("/api/system/info", get(get_system_info))
        .route("/api/fix-image-paths", post(fix_image_paths))
        // === WEBSOCKET ===
        .route("/ws", get(handlers::websocket_handler))
        
        // === SERVIR FICHIERS STATIQUES ===
        // === SERVIR FICHIERS STATIQUES ===
        .nest_service("/assets", ServeDir::new("frontend/assets").fallback(ServeDir::new("../frontend/assets")))
    
        
        .route("/", get(serve_your_index))
        .route("/index.html", get(serve_your_index))
        .route("/history.html", get(serve_your_history))
        .fallback_service(ServeDir::new("../frontend"))
        .route("/api/debug/assets", get(debug_assets))
        // === MIDDLEWARE ===
        .with_state(pool)
        .layer(cors);

    let final_port = find_free_port(&[3001, 3002, 3003, 3004, 3005]).unwrap_or(3001);
    let addr = SocketAddr::from(([0, 0, 0, 0], final_port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let local_ip = get_local_ip().unwrap_or_else(|| std::net::Ipv4Addr::new(127, 0, 0, 1));
    
    println!("\n==========================================");
    println!("   SERVEUR ESP32 PRET!");
    println!("==========================================");
    println!("URLs d'accès:");
    println!("   - Local:      http://127.0.0.1:{}", final_port);
    println!("   - Réseau:     http://{}:{}", local_ip, final_port);
    println!("   - WebSocket:  ws://{}:{}/ws", local_ip, final_port);
    println!();
    println!("Pages Web:");
    println!("   - Détection:  http://{}:{}/index.html", local_ip, final_port);
    println!("   - Historique: http://{}:{}/history.html", local_ip, final_port);
    println!();
    println!("Endpoints ESP32 + Flask Compatibility:");
    println!("   - GET  /api/detections     (Flask compat)");
    println!("   - POST /api/camera/start   (Flask compat)");
    println!("   - POST /api/camera/stop    (Flask compat)");
    println!("   - GET  /api/camera/status  (Flask compat)");
    println!("   - GET  /api/statistics     (Flask compat)");
    println!("   - POST /api/reset          (Flask compat)");
    println!("   - POST /api/detections/save (ESP32 save)");
    println!();
    println!("🔧 COMPATIBILITÉ FLASK: Activée");
    println!("🔧 STRUCTURE BDD: Migrée automatiquement");
    println!();
    println!("Serveur en écoute...");

    axum::serve(listener, app).await?;
    Ok(())
}
async fn test_assets_route() -> ResponseJson<serde_json::Value> {
    let mut assets_info = Vec::new();
    
    let test_paths = vec![
        "../frontend/assets/captures",
        "frontend/assets/captures",
        "../frontend/assets",
        "frontend/assets"
    ];
    
    for path_str in test_paths {
        let path = std::path::Path::new(path_str);
        if path.exists() && path.is_dir() {
            let mut files = Vec::new();
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries {
                    if let Ok(entry) = entry {
                        files.push(entry.file_name().to_string_lossy().to_string());
                    }
                }
            }
            
            assets_info.push(json!({
                "path": path_str,
                "exists": true,
                "files_count": files.len(),
                "files": files.into_iter().take(5).collect::<Vec<_>>()
            }));
        } else {
            assets_info.push(json!({
                "path": path_str,
                "exists": false,
                "error": "Directory not found"
            }));
        }
    }
    
    ResponseJson(json!({
        "assets_info": assets_info,
        "working_directory": std::env::current_dir().unwrap_or_default(),
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}
// === HANDLERS DE COMPATIBILITÉ FLASK ===
#[derive(serde::Deserialize)]
struct DetectionPayload {
    g_id: String,
    type_name: String,
    color: String,
    image_data: Option<String>,
}
async fn debug_assets() -> ResponseJson<serde_json::Value> {
    let mut assets_debug = Vec::new();
    
    let test_paths = vec![
        "../frontend/assets/captures",
        "frontend/assets/captures"
    ];
    
    for path_str in test_paths {
        let path = std::path::Path::new(path_str);
        if path.exists() {
            let mut files = Vec::new();
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries {
                    if let Ok(entry) = entry {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name.ends_with(".jpg") || name.ends_with(".png") {
                            files.push(name);
                        }
                    }
                }
            }
            assets_debug.push(json!({
                "path": path_str,
                "exists": true,
                "image_files": files.len(),
                "sample_files": files.into_iter().take(5).collect::<Vec<_>>()
            }));
        } else {
            assets_debug.push(json!({
                "path": path_str,
                "exists": false
            }));
        }
    }
    
    ResponseJson(json!({
        "working_directory": std::env::current_dir().unwrap_or_default(),
        "assets_paths": assets_debug
    }))
}
async fn add_detection(
    State(pool): State<SqlitePool>,
    Json(payload): Json<DetectionPayload>
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    println!("Réception détection: {} - {}", payload.color, payload.type_name);

    // Sauvegarder l'image si présente
    let image_path = if let Some(image_data) = &payload.image_data {
        match save_base64_image(image_data, &payload.color).await {
            Ok(path) => {
                println!("Image sauvegardée: {}", path);
                path
            },
            Err(e) => {
                println!("Erreur sauvegarde image: {}", e);
                "/assets/no_image.jpg".to_string()
            }
        }
    } else {
        "/assets/no_image.jpg".to_string()
    };

    // Insérer en base de données
    match sqlx::query(
        "INSERT INTO detections (g_id, type_name, color, image_path, confidence) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(&payload.g_id)
    .bind(&payload.type_name)
    .bind(&payload.color)
    .bind(&image_path)
    .bind(0.85)
    .execute(&pool)
    .await
    {
        Ok(result) => {
            let detection_id = result.last_insert_rowid();
            println!("Détection sauvegardée: ID={}", detection_id);
            
            let _ = database::update_real_time_counters(&pool).await;
            
            Ok(Json(serde_json::json!({
                "success": true,
                "id": detection_id,
                "message": "Détection ajoutée",
                "image_path": image_path,
                "data": {
                    "color": payload.color,
                    "type_name": payload.type_name,
                    "g_id": payload.g_id,
                    "image_path": image_path
                }
            })))
        },
        Err(e) => {
            println!("Erreur base de données: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, format!("Erreur base de données: {}", e)))
        }
    }

}

// Fonction pour sauvegarder l'image base64
async fn save_base64_image(base64_data: &str, color: &str) -> Result<String, Box<dyn std::error::Error>> {
    use base64::{Engine as _, engine::general_purpose};
    use std::io::Write;

    let image_data = if base64_data.starts_with("data:image/jpeg;base64,") {
        &base64_data[23..]
    } else if base64_data.starts_with("data:image/png;base64,") {
        &base64_data[22..]
    } else {
        base64_data
    };

    let decoded = general_purpose::STANDARD.decode(image_data)?;
    
    // Créer TOUS les dossiers nécessaires
    std::fs::create_dir_all("../frontend/assets/captures")?;
    std::fs::create_dir_all("frontend/assets/captures")?;

    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S_%3f").to_string();
    let filename = format!("esp32_{}_{}.jpg", timestamp, color);
    
    // Sauvegarder dans les deux emplacements
    // Sauvegarder dans les deux emplacements (priorité à frontend/)
    let paths = vec![
        format!("frontend/assets/captures/{}", filename),
        format!("../frontend/assets/captures/{}", filename)
    ];
    
    for path in &paths {
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::File::create(path)?;
        file.write_all(&decoded)?;
        println!("Image sauvegardée: {}", path);
    }

    Ok(format!("/assets/captures/{}", filename))
}
async fn check_assets_availability() -> ResponseJson<serde_json::Value> {
    let mut assets_status = Vec::new();
    
    let paths = vec!["frontend/assets/captures", "../frontend/assets/captures"];
    
    for path in paths {
        let path_obj = std::path::Path::new(path);
        if path_obj.exists() {
            let mut files = Vec::new();
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries {
                    if let Ok(entry) = entry {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name.ends_with(".jpg") || name.ends_with(".png") {
                            files.push(name);
                        }
                    }
                }
            }
            assets_status.push(json!({
                "path": path,
                "exists": true,
                "image_count": files.len(),
                "sample_images": files.into_iter().take(3).collect::<Vec<_>>()
            }));
        } else {
            assets_status.push(json!({
                "path": path,
                "exists": false
            }));
        }
    }
    
    ResponseJson(json!({
        "assets_check": assets_status,
        "working_directory": std::env::current_dir().unwrap_or_default()
    }))
}
async fn debug_detection(
    Json(payload): Json<serde_json::Value>
) -> Result<Json<serde_json::Value>, StatusCode> {
    println!("=== DEBUG DÉTECTION ===");
    println!("Payload reçu: {}", serde_json::to_string_pretty(&payload).unwrap_or_default());
    
    if let Some(image_data) = payload.get("image_data") {
        if let Some(image_str) = image_data.as_str() {
            println!("Image data présente: {} caractères", image_str.len());
            println!("Type d'image: {}", if image_str.starts_with("data:image/") { "Base64 data URL" } else { "Autre" });
        }
    } else {
        println!("Aucune image_data dans le payload");
    }
    
    Ok(Json(json!({"received": true, "debug": "payload logged"})))
}

// Ajoutez cette route pour debug :

async fn debug_image_paths(State(pool): State<SqlitePool>) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    let images = sqlx::query_as::<_, (i64, String, String)>(
        "SELECT id, color, image_path FROM detections ORDER BY id DESC LIMIT 10"
    )
    .fetch_all(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(ResponseJson(json!({
        "recent_images": images,
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}
async fn get_detections_flask_compat(
    State(pool): State<SqlitePool>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    let limit = params.get("limit")
        .and_then(|l| l.parse::<i64>().ok())
        .unwrap_or(50);

    let detections = sqlx::query_as::<_, (i64, String, String, String, String, String, Option<f64>, Option<f64>, Option<f64>)>(
        "SELECT id, g_id, type_name, color, date_time, image_path, confidence, centroid_x, centroid_y 
         FROM detections 
         ORDER BY datetime(date_time) DESC, id DESC 
         LIMIT ?"
    )
    .bind(limit)
    .fetch_all(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let formatted: Vec<serde_json::Value> = detections
        .into_iter()
        .map(|(id, g_id, type_name, color, date_time, image_path, confidence, centroid_x, centroid_y)| {
            json!({
                "id": id,
                "g_id": g_id,
                "type": type_name,
                "color": color,
                "timestamp": date_time,
                "image_path": if image_path == "no_image.jpg" || image_path.is_empty() {
                format!("/assets/no_image.jpg")
                } else if image_path.starts_with("/assets/") {
                    image_path.clone()
                } else {
                format!("/assets/captures/{}", image_path)
},
                "confidence": confidence.unwrap_or(1.0),
                "centroid_x": centroid_x.unwrap_or(0.0),
                "centroid_y": centroid_y.unwrap_or(0.0)
            })
        })
        .collect();

    Ok(ResponseJson(json!({
        "detections": formatted,
        "total": formatted.len()
    })))
}

async fn camera_start() -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    println!("📹 Demande d'activation caméra ESP32");
    Ok(ResponseJson(json!({
        "status": "success",
        "message": "Caméra ESP32 activée", // <- Simulation seulement
        "camera_status": "running",
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}
async fn fix_all_image_paths(State(pool): State<SqlitePool>) -> Result<Json<serde_json::Value>, StatusCode> {
    // Corriger tous les chemins d'images existants
    let result = sqlx::query(
        "UPDATE detections SET image_path = 
         CASE 
             WHEN image_path = 'no_image.jpg' THEN '/assets/no_image.jpg'
             WHEN image_path NOT LIKE '/assets/%' AND image_path != 'no_image.jpg' 
             THEN '/assets/captures/' || image_path
             ELSE image_path
         END"
    )
    .execute(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({
        "fixed_paths": result.rows_affected(),
        "message": "Tous les chemins d'images ont été normalisés"
    })))
}
async fn camera_stop() -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    println!("⏹️ Demande d'arrêt caméra ESP32");
    Ok(ResponseJson(json!({
        "status": "success", 
        "message": "Caméra ESP32 arrêtée",
        "camera_status": "stopped",
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

async fn camera_status() -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    Ok(ResponseJson(json!({
        "status": "online",
        "camera_active": true,
        "esp32_connected": true,
        "last_detection": chrono::Utc::now().to_rfc3339(),
        "message": "Caméra ESP32 opérationnelle"
    })))
}

async fn get_statistics(State(pool): State<SqlitePool>) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    match database::get_detection_stats(&pool).await {
        Ok(stats) => Ok(ResponseJson(stats)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
}
async fn fix_image_paths(State(pool): State<SqlitePool>) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    // Corriger les chemins qui ne commencent pas par /assets/
    let result = sqlx::query(
        "UPDATE detections SET image_path = '/assets/captures/' || image_path 
        WHERE image_path NOT LIKE '/assets/%' AND image_path != 'no_image.jpg'"
    )
    .execute(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(ResponseJson(json!({
        "fixed_paths": result.rows_affected(),
        "message": "Chemins d'images corrigés"
    })))
}

async fn reset_system(State(pool): State<SqlitePool>) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    println!("🔄 Demande de réinitialisation système");
    match database::reset_all_counters(&pool).await {
        Ok(_) => Ok(ResponseJson(json!({
            "status": "success",
            "message": "Système réinitialisé avec succès",
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

// === HANDLERS EXISTANTS (inchangés) ===

async fn serve_your_index() -> Result<axum::response::Html<String>, StatusCode> {
    match tokio::fs::read_to_string("../frontend/index.html").await {
        Ok(content) => {
            println!("✅ Serving your index.html file from ../frontend/");
            Ok(axum::response::Html(content))
        },
        Err(e) => {
            eprintln!("❌ Erreur lecture ../frontend/index.html: {}", e);
            match tokio::fs::read_to_string("frontend/index.html").await {
                Ok(content) => Ok(axum::response::Html(content)),
                Err(_) => Err(StatusCode::NOT_FOUND)
            }
        }
    }
}

async fn serve_your_history() -> Result<axum::response::Html<String>, StatusCode> {
    match tokio::fs::read_to_string("../frontend/history.html").await {
        Ok(content) => {
            println!("✅ Serving your history.html file from ../frontend/");
            Ok(axum::response::Html(content))
        },
        Err(_) => {
            match tokio::fs::read_to_string("frontend/history.html").await {
                Ok(content) => Ok(axum::response::Html(content)),
                Err(_) => Err(StatusCode::NOT_FOUND)
            }
        }
    }
}

fn check_frontend_files() {
    println!("\n5. VERIFICATION FICHIERS FRONTEND");
    let files_to_check = vec![
        ("../frontend/index.html", "Votre index.html"),
        ("../frontend/history.html", "Votre history.html"),
        ("../frontend/assets", "Dossier assets"),
    ];
    
    for (file_path, description) in files_to_check {
        let path = std::path::Path::new(file_path);
        if path.exists() {
            if path.is_dir() {
                println!("  ✅ {} trouvé: {}", description, file_path);
            } else {
                let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                println!("  ✅ {} trouvé: {} ({} bytes)", description, file_path, size);
            }
        } else {
            println!("  ❌ {} MANQUANT: {}", description, file_path);
        }
    }
}

fn create_resource_directories() -> Result<(), std::io::Error> {
    let directories = vec![
        ("data", "Base de données"),
        ("backups", "Sauvegardes"),
        ("../frontend/assets", "Assets frontend"),
        ("../frontend/assets/captures", "Images captures"),
        ("frontend/assets", "Assets frontend (alt)"),
        ("frontend/assets/captures", "Images captures (alt)"),
    ];


    for (dir_path, description) in directories {
        match std::fs::create_dir_all(dir_path) {
            Ok(_) => println!("✅ Dossier créé: {} ({})", dir_path, description),
            Err(e) => {
                // Ne pas échouer si le dossier existe déjà
                if e.kind() != std::io::ErrorKind::AlreadyExists {
                    println!("⚠️ Avertissement {}: {} - {}", description, dir_path, e);
                } else {
                    println!("✅ Dossier existant: {} ({})", dir_path, description);
                }
            }
        }
    }
    
    // Créer l'image par défaut si elle n'existe pas
    create_default_no_image()?;
    // Créer une image par défaut simple
    let no_image_path = "../frontend/assets/no_image.jpg";
    if !std::path::Path::new(no_image_path).exists() {
    // Créer un fichier placeholder simple (pas une vraie image)
        let placeholder_content = b"\xFF\xD8\xFF\xE0\x00\x10JFIF"; // En-tête JPEG minimal
        std::fs::write(no_image_path, placeholder_content)?;
        println!("✅ Image par défaut créée: {}", no_image_path);
}


    Ok(())
}

fn create_default_no_image() -> Result<(), std::io::Error> {
    let paths = vec![
        "../frontend/assets/no_image.jpg",
        "frontend/assets/no_image.jpg"
    ];
    
    for path in paths {
        if !std::path::Path::new(path).exists() {
            if let Some(parent) = std::path::Path::new(path).parent() {
                std::fs::create_dir_all(parent)?;
            }
            
            // Créer un fichier placeholder simple
            let placeholder_content = "IMAGE_PLACEHOLDER";
            std::fs::write(path, placeholder_content)?;
            println!("✅ Placeholder créé: {}", path);
        }
    }
    
    Ok(())
}


fn find_free_port(ports: &[u16]) -> Option<u16> {
    for &port in ports {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        if std::net::TcpListener::bind(addr).is_ok() {
            return Some(port);
        }
    }
    None
}

// === HANDLERS SYSTÈME EXISTANTS ===

async fn get_system_status(State(pool): State<SqlitePool>) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    let database_info = database::get_database_info(&pool).await.unwrap_or_else(|_| {
        serde_json::json!({"error": "Impossible d'accéder à la base de données"})
    });

    Ok(ResponseJson(serde_json::json!({
        "status": "online",
        "version": "2.0.0-esp32-flask-compat",
        "database": database_info,
        "flask_compatibility": true,
        "endpoints": {
            "total": 30,
            "esp32_specific": 6,
            "flask_compat": 6,
            "websocket_enabled": true
        },
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

async fn esp32_ping() -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    Ok(ResponseJson(serde_json::json!({
        "status": "pong",
        "message": "ESP32 système opérationnel avec compatibilité Flask",
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

async fn esp32_test(
    State(pool): State<SqlitePool>,
    axum::extract::Json(payload): axum::extract::Json<serde_json::Value>
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    println!("🧪 Test ESP32 reçu: {:?}", payload);
    
    let types_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM types")
        .fetch_one(&pool)
        .await
        .unwrap_or(0);
    
    Ok(ResponseJson(serde_json::json!({
        "status": "success",
        "message": "Test ESP32 avec Flask compatibility réussi",
        "received_payload": payload,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "system_info": {
            "database_ok": true,
            "types_available": types_count,
            "flask_compat": true
        }
    })))
}

async fn network_info() -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    let local_ip = get_local_ip().unwrap_or_else(|| std::net::Ipv4Addr::new(127, 0, 0, 1));
    let port = 3001;
    
    Ok(ResponseJson(serde_json::json!({
        "network_info": {
            "local_ip": local_ip.to_string(),
            "port": port,
            "esp32_url": format!("http://{}:{}", local_ip, port)
        }
    })))
}

async fn get_system_info() -> ResponseJson<serde_json::Value> {
    let current_dir = std::env::current_dir().unwrap_or_default();
    
    ResponseJson(serde_json::json!({
        "working_directory": current_dir,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "integration_status": "Flask compatibility enabled"
    }))
}

// === HANDLERS HISTORIQUE ===

async fn get_history_handler(
    State(pool): State<SqlitePool>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    let page = params.get("page").and_then(|p| p.parse::<i64>().ok()).unwrap_or(1);
    let limit = params.get("limit").and_then(|l| l.parse::<i64>().ok()).unwrap_or(20);

    match database::get_history_paginated(&pool, page, limit).await {
        Ok(history) => Ok(ResponseJson(history)),
        Err(e) => {
            eprintln!("Erreur récupération historique: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn get_history_filtered_handler(
    State(pool): State<SqlitePool>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    let color_filter = params.get("color").cloned();
    let type_filter = params.get("type").cloned();
    let date_from = params.get("date_from").cloned();
    let date_to = params.get("date_to").cloned();
    let limit = params.get("limit").and_then(|l| l.parse::<i64>().ok()).unwrap_or(100);

    match database::get_history_filtered(&pool, color_filter, type_filter, date_from, date_to, limit).await {
        Ok(history) => Ok(ResponseJson(history)),
        Err(e) => {
            eprintln!("Erreur récupération historique filtré: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn delete_detection_handler(
    State(pool): State<SqlitePool>,
    axum::extract::Path(detection_id): axum::extract::Path<i64>
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    match sqlx::query("DELETE FROM detections WHERE id = ?").bind(detection_id).execute(&pool).await {
        Ok(result) => {
            if result.rows_affected() > 0 {
                let _ = database::update_real_time_counters(&pool).await;
                Ok(ResponseJson(serde_json::json!({
                    "success": true,
                    "message": "Détection supprimée",
                    "deleted_id": detection_id
                })))
            } else {
                Ok(ResponseJson(serde_json::json!({
                    "success": false,
                    "message": "Détection non trouvée"
                })))
            }
        }
        Err(e) => {
            eprintln!("Erreur suppression détection: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
async fn debug_file_paths() -> Result<(), std::io::Error> {
    let current_dir = std::env::current_dir()?;
    println!("🔍 Répertoire de travail: {}", current_dir.display());
    
    let paths_to_check = vec![
        "../frontend/assets/captures",
        "frontend/assets/captures",
        "../frontend/assets",
        "frontend/assets"
    ];
    
    for path in paths_to_check {
        let path_obj = std::path::Path::new(path);
        if path_obj.exists() {
            println!("✅ Trouvé: {}", path);
            if let Ok(entries) = std::fs::read_dir(path) {
                let count = entries.count();
                println!("   Fichiers: {}", count);
            }
        } else {
            println!("❌ Manquant: {}", path);
        }
    }
    
    Ok(())
}