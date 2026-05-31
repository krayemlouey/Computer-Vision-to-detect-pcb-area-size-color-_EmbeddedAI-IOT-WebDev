use axum::{
    extract::{Query, State, WebSocketUpgrade, ws::WebSocket},
    http::StatusCode,
    Json,
    response::{Json as ResponseJson, Response},
};
use sqlx::SqlitePool;
use std::collections::HashMap;
use base64::{Engine as _, engine::general_purpose};
use std::fs;
use uuid::Uuid;
use tokio::sync::broadcast;
use futures_util::{sink::SinkExt, stream::StreamExt};

// Import des modèles
use crate::models::*;
use crate::database;

// Canal pour notifications en temps réel
static mut NOTIFICATION_SENDER: Option<broadcast::Sender<String>> = None;

pub fn init_notifications() {
    unsafe {
        let (tx, _rx) = broadcast::channel(100);
        NOTIFICATION_SENDER = Some(tx);
    }
}

fn notify_clients(message: &str) {
    unsafe {
        if let Some(sender) = &NOTIFICATION_SENDER {
            let _ = sender.send(message.to_string());
        }
    }
}

// === HANDLERS D'AUTHENTIFICATION ===

pub async fn login(Json(payload): Json<LoginRequest>) -> Result<ResponseJson<LoginResponse>, StatusCode> {
    if payload.username == "admin" && payload.password == "admin123" {
        Ok(ResponseJson(LoginResponse {
            success: true,
            message: "Connexion réussie".to_string(),
            token: Some("esp32_token_2024".to_string()),
        }))
    } else {
        Ok(ResponseJson(LoginResponse {
            success: false,
            message: "Identifiants invalides".to_string(),
            token: None,
        }))
    }
}

pub async fn verify_auth() -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    Ok(ResponseJson(serde_json::json!({
        "authenticated": true,
        "message": "Token valide",
        "esp32_ready": true
    })))
}

// === HANDLERS DE DÉTECTION ===

pub async fn save_detection(
    State(pool): State<SqlitePool>,
    Json(payload): Json<DetectionRequest>,
) -> Result<ResponseJson<DetectionResponse>, StatusCode> {
    
    println!("Nouvelle détection reçue: G_ID={}, Type={}, Couleur={}", 
             payload.g_id, payload.type_name, payload.color);
    
    let normalized_color = normalize_color_name(&payload.color);
    let type_name = if payload.type_name.is_empty() {
        format!("Type_{}", payload.g_id)
    } else {
        payload.type_name.clone()
    };

    let (centroid_x, centroid_y) = if let Some(ref centroid) = payload.centroid {
        if centroid.len() >= 2 {
            (centroid[0], centroid[1])
        } else {
            (0.0, 0.0)
        }
    } else {
        (0.0, 0.0)
    };

    let image_path = if !payload.image_data.is_empty() {
        match save_image_from_base64(&payload.image_data, &normalized_color).await {
            Ok(path) => path,
            Err(e) => {
                eprintln!("Erreur sauvegarde image: {}", e);
                "no_image.jpg".to_string()
            }
        }
    } else {
        "no_image.jpg".to_string()
    };

    let result = sqlx::query(
        r#"INSERT INTO detections (g_id, type_name, color, image_path, date_time, centroid_x, centroid_y, confidence) 
           VALUES (?1, ?2, ?3, ?4, datetime('now', 'localtime'), ?5, ?6, ?7)"#
    )
    .bind(&payload.g_id)
    .bind(&type_name)
    .bind(&normalized_color)
    .bind(&image_path)
    .bind(centroid_x)
    .bind(centroid_y)
    .bind(payload.confidence.unwrap_or(95.0))
    .execute(&pool)
    .await;

    match result {
        Ok(query_result) => {
            let detection_id = query_result.last_insert_rowid();
            
            let detection_row = sqlx::query_as::<_, (i64, String, String, String, String, String, f64, f64, f64)>(
                "SELECT id, g_id, type_name, color, date_time, image_path, centroid_x, centroid_y, confidence FROM detections WHERE id = ?1"
            )
            .bind(detection_id)
            .fetch_one(&pool)
            .await;

            match detection_row {
                Ok((id, g_id, type_name, color, date_time, image_path, centroid_x, centroid_y, confidence)) => {
                    println!("Détection sauvegardée: ID {} - {} ({}) - {}", id, type_name, color, date_time);
                    
                    let notification = serde_json::json!({
                        "type": "new_detection",
                        "data": {
                            "id": id,
                            "g_id": g_id,
                            "type_name": type_name,
                            "color": color,
                            "date_time": date_time,
                            "image_path": format!("/assets/captures/{}", image_path),
                            "centroid_x": centroid_x,
                            "centroid_y": centroid_y,
                            "confidence": confidence
                        },
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    });
                    
                    notify_clients(&notification.to_string());
                    
                    Ok(ResponseJson(DetectionResponse {
                        id,
                        g_id,
                        type_name,
                        color,
                        date_time: Some(date_time),
                        image_path: format!("/assets/captures/{}", image_path),
                        confidence: Some(confidence),
                    }))
                }
                Err(e) => {
                    eprintln!("Erreur récupération détection: {}", e);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
        Err(e) => {
            eprintln!("Erreur insertion détection: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// === HANDLERS ESP32 SPÉCIALISÉS ===

pub async fn update_color_mapping_esp32(
    State(pool): State<SqlitePool>,
    Json(payload): Json<ESP32UpdateRequest>,
) -> Result<ResponseJson<ESP32UpdateResponse>, StatusCode> {
    
    let source = &payload.source;
    println!("📡 Mise à jour mapping depuis {} : couleur={}, g_id={}, type={}",
             source, payload.color, payload.g_id, payload.type_name);

    let validation_result = validate_esp32_input(&payload);
    if !validation_result.is_empty() {
        return Ok(ResponseJson(ESP32UpdateResponse {
            success: false,
            message: format!("Validation échouée: {}", validation_result.join(", ")),
            updated_color: None,
            old_mapping: None,
            new_mapping: None,
        }));
    }

    let existing_gid = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM types WHERE g_id = ?1"
    )
    .bind(&payload.g_id)
    .fetch_one(&pool)
    .await;

    if let Ok(count) = existing_gid {
        if count > 0 {
            return Ok(ResponseJson(ESP32UpdateResponse {
                success: false,
                message: format!("G_ID {} déjà utilisé", payload.g_id),
                updated_color: None,
                old_mapping: None,
                new_mapping: None,
            }));
        }
    }

    let old_mapping_result = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT g_id, type_name, color FROM types WHERE LOWER(color) = LOWER(?1)"
    )
    .bind(&payload.color)
    .fetch_optional(&pool)
    .await;

    let old_mapping = match old_mapping_result {
        Ok(Some((old_g_id, old_type_name, old_color))) => {
            Some(ColorMapping {
                color: old_color.unwrap_or_else(|| payload.color.clone()),
                g_id: old_g_id,
                type_name: old_type_name,
            })
        }
        _ => None,
    };

    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            eprintln!("Erreur début transaction: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    if let Err(e) = sqlx::query("DELETE FROM types WHERE LOWER(color) = LOWER(?1)")
        .bind(&payload.color)
        .execute(&mut *tx)
        .await
    {
        eprintln!("Erreur suppression ancien mapping: {}", e);
        let _ = tx.rollback().await;
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    if let Err(e) = sqlx::query("INSERT INTO types (g_id, type_name, color) VALUES (?1, ?2, ?3)")
        .bind(&payload.g_id)
        .bind(&payload.type_name)
        .bind(&payload.color.to_lowercase())
        .execute(&mut *tx)
        .await
    {
        eprintln!("Erreur insertion nouveau mapping: {}", e);
        let _ = tx.rollback().await;
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    if let Err(e) = tx.commit().await {
        eprintln!("Erreur commit transaction: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    let new_mapping = ColorMapping {
        color: payload.color.clone(),
        g_id: payload.g_id.clone(),
        type_name: payload.type_name.clone(),
    };

    let notification = serde_json::json!({
        "type": "mapping_update",
        "data": {
            "color": payload.color,
            "g_id": payload.g_id,
            "type_name": payload.type_name,
            "old_mapping": old_mapping,
            "new_mapping": new_mapping
        },
        "timestamp": chrono::Utc::now().to_rfc3339()
    });
    
    notify_clients(&notification.to_string());

    Ok(ResponseJson(ESP32UpdateResponse {
        success: true,
        message: format!("Mapping ESP32 mis à jour: {} -> {}", payload.color, payload.type_name),
        updated_color: Some(payload.color),
        old_mapping,
        new_mapping: Some(new_mapping),
    }))
}

pub async fn validate_esp32_data(
    State(pool): State<SqlitePool>,
    Json(payload): Json<ESP32UpdateRequest>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    
    let validation_errors = validate_esp32_input(&payload);

    let mut final_errors = validation_errors;
    if final_errors.is_empty() {
        let existing_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM types WHERE g_id = ?1"
        )
        .bind(&payload.g_id)
        .fetch_one(&pool)
        .await
        .unwrap_or(0);

        if existing_count > 0 {
            final_errors.push(format!("G_ID {} déjà utilisé", payload.g_id));
        }
    }

    let is_valid = final_errors.is_empty();

    Ok(ResponseJson(serde_json::json!({
        "valid": is_valid,
        "errors": final_errors,
        "g_id": payload.g_id,
        "color": payload.color,
        "type_name": payload.type_name,
        "message": if is_valid { "Données ESP32 valides" } else { "Erreurs détectées" }
    })))
}

pub async fn get_current_mappings_esp32(State(pool): State<SqlitePool>) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    let mappings_result = get_color_mappings(State(pool)).await;
    
    match mappings_result {
        Ok(ResponseJson(mappings)) => {
            let color_map: HashMap<String, serde_json::Value> = mappings
                .into_iter()
                .map(|mapping| {
                    (mapping.color.clone(), serde_json::json!({
                        "g_id": mapping.g_id,
                        "type_name": mapping.type_name
                    }))
                })
                .collect();

            Ok(ResponseJson(serde_json::json!({
                "success": true,
                "mappings": color_map,
                "count": color_map.len(),
                "timestamp": chrono::Utc::now().to_rfc3339()
            })))
        }
        Err(status) => Err(status),
    }
}

pub async fn get_esp32_devices() -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    Ok(ResponseJson(serde_json::json!({
        "devices": [],
        "count": 0,
        "last_update": chrono::Utc::now().to_rfc3339(),
        "message": "Aucun ESP32 trackable pour l'instant"
    })))
}

// === HANDLERS EXISTANTS AMÉLIORÉS ===

pub async fn get_history(
    State(pool): State<SqlitePool>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<ResponseJson<HistoryResponse>, StatusCode> {
    
    let limit: i64 = params
        .get("limit")
        .and_then(|l| l.parse().ok())
        .unwrap_or(50);

    let offset: i64 = params
        .get("offset")
        .and_then(|o| o.parse().ok())
        .unwrap_or(0);

    let page: i64 = params
        .get("page")
        .and_then(|p| p.parse().ok())
        .unwrap_or(1);

    let per_page: i64 = params
        .get("per_page")
        .and_then(|pp| pp.parse().ok())
        .unwrap_or(50);

    println!("Chargement historique: limit={}, offset={}", limit, offset);

    let detections_rows = sqlx::query_as::<_, (i64, String, String, String, String, String, f64, f64, f64)>(
        "SELECT id, g_id, type_name, color, date_time, image_path, centroid_x, centroid_y, confidence
         FROM detections 
         ORDER BY id DESC 
         LIMIT ?1 OFFSET ?2"
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&pool)
    .await;

    let total_result = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM detections")
        .fetch_one(&pool)
        .await;

    let counters_result = sqlx::query_as::<_, (String, i64)>(
        "SELECT color, COUNT(*) as count FROM detections GROUP BY color ORDER BY count DESC"
    )
    .fetch_all(&pool)
    .await;

    match (detections_rows, total_result, counters_result) {
        (Ok(rows), Ok(total), Ok(counter_rows)) => {
            println!("Historique chargé: {} détections sur {}", rows.len(), total);
            
            let detections: Vec<Detection> = rows
                .into_iter()
                .map(|(id, g_id, type_name, color, date_time, image_path, centroid_x, centroid_y, confidence)| Detection {
                    id,
                    g_id,
                    type_name,
                    color,
                    date_time: Some(date_time),
                    image_path: format!("/assets/captures/{}", image_path),
                    centroid_x: Some(centroid_x),
                    centroid_y: Some(centroid_y),
                    confidence: Some(confidence),
                })
                .collect();

            let mut color_counts = HashMap::new();
            for (color, count) in counter_rows {
                color_counts.insert(color, count);
            }

            let counters = CountersResponse {
                rouge: *color_counts.get("rouge").unwrap_or(&0),
                vert: *color_counts.get("vert").unwrap_or(&0),
                bleu: *color_counts.get("bleu").unwrap_or(&0),
                jaune: *color_counts.get("jaune").unwrap_or(&0),
                noir: *color_counts.get("noir").unwrap_or(&0),
                total,
                last_update: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
            };

            Ok(ResponseJson(HistoryResponse {
                detections,
                total,
                page: Some(page),
                per_page: Some(per_page),
                counters: Some(counters),
            }))
        }
        (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
            eprintln!("Erreur récupération historique: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// === HANDLER COMPTEURS - CORRECTION PRINCIPALE ===

pub async fn get_real_time_counters_handler(
    State(pool): State<SqlitePool>,
) -> Result<ResponseJson<CountersResponse>, StatusCode> {
    println!("🔄 Récupération des compteurs en temps réel...");
    
    let counters_result = sqlx::query_as::<_, (String, i64)>(
        "SELECT color, COUNT(*) as count FROM detections GROUP BY color ORDER BY count DESC"
    )
    .fetch_all(&pool)
    .await;

    let total_result = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM detections")
        .fetch_one(&pool)
        .await;

    match (counters_result, total_result) {
        (Ok(counter_rows), Ok(total)) => {
            let mut color_counts = HashMap::new();
            for (color, count) in counter_rows {
                color_counts.insert(color, count);
            }

            let counters = CountersResponse {
                rouge: *color_counts.get("rouge").unwrap_or(&0),
                vert: *color_counts.get("vert").unwrap_or(&0),
                bleu: *color_counts.get("bleu").unwrap_or(&0),
                jaune: *color_counts.get("jaune").unwrap_or(&0),
                noir: *color_counts.get("noir").unwrap_or(&0),
                total,
                last_update: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
            };

            println!("✅ Compteurs récupérés: Total={}, Rouge={}, Vert={}, Bleu={}, Jaune={}, Noir={}", 
                    total, counters.rouge, counters.vert, counters.bleu, counters.jaune, counters.noir);
            
            Ok(ResponseJson(counters))
        }
        _ => {
            eprintln!("❌ Erreur récupération compteurs");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn reset_counters_handler(
    State(pool): State<SqlitePool>,
    Json(payload): Json<serde_json::Value>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    let confirm = payload.get("confirm").and_then(|c| c.as_bool()).unwrap_or(false);
    
    if !confirm {
        return Ok(ResponseJson(serde_json::json!({
            "success": false,
            "message": "Confirmation requise pour réinitialiser les compteurs"
        })));
    }

    let result = sqlx::query("DELETE FROM detections")
        .execute(&pool)
        .await;

    match result {
        Ok(query_result) => {
            let deleted_count = query_result.rows_affected();
            println!("Compteurs réinitialisés: {} détections supprimées", deleted_count);
            
            let notification = serde_json::json!({
                "type": "counters_reset",
                "data": {
                    "deleted_count": deleted_count,
                    "counters": {
                        "rouge": 0,
                        "vert": 0,
                        "bleu": 0,
                        "jaune": 0,
                        "noir": 0,
                        "total": 0
                    }
                },
                "timestamp": chrono::Utc::now().to_rfc3339()
            });
            
            notify_clients(&notification.to_string());
            
            Ok(ResponseJson(serde_json::json!({
                "success": true,
                "message": format!("Compteurs réinitialisés - {} détections supprimées", deleted_count),
                "deleted_count": deleted_count
            })))
        }
        Err(e) => {
            eprintln!("Erreur réinitialisation compteurs: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// === AUTRES HANDLERS ===

pub async fn export_history(
    State(pool): State<SqlitePool>,
    Json(payload): Json<ExportRequest>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    
    let format = payload.format.unwrap_or_else(|| "csv".to_string());
    
    let detections_result = sqlx::query_as::<_, (i64, String, String, String, String, String, f64, f64, f64)>(
        "SELECT id, g_id, type_name, color, date_time, image_path, centroid_x, centroid_y, confidence FROM detections ORDER BY date_time DESC"
    )
    .fetch_all(&pool)
    .await;

    match detections_result {
        Ok(rows) => {
            let mut content = String::new();
            
            if format == "csv" {
                content.push_str("ID,G_ID,Type,Couleur,Date,Image,Centroid_X,Centroid_Y,Confidence\n");
                for (id, g_id, type_name, color, date_time, image_path, centroid_x, centroid_y, confidence) in &rows {
                    content.push_str(&format!("{},{},{},{},{},{},{},{},{}\n", 
                                            id, g_id, type_name, color, date_time, image_path, centroid_x, centroid_y, confidence));
                }
            } else {
                content.push_str("=== HISTORIQUE DES DÉTECTIONS ESP32 ===\n\n");
                for (id, g_id, type_name, color, date_time, _, centroid_x, centroid_y, confidence) in &rows {
                    content.push_str(&format!("ID: {} | G_ID: {} | Type: {} | Couleur: {} | Date: {} | Position: ({:.1}, {:.1}) | Confiance: {:.1}%\n", 
                                            id, g_id, type_name, color, date_time, centroid_x, centroid_y, confidence));
                }
            }
            
            let filename = format!("export_esp32_{}_{}.{}", 
                chrono::Utc::now().format("%Y%m%d_%H%M%S"), 
                rows.len(), 
                format
            );
            
            Ok(ResponseJson(serde_json::json!({
                "success": true,
                "filename": filename,
                "count": rows.len(),
                "content": content,
                "format": format
            })))
        }
        Err(e) => {
            eprintln!("Erreur export: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn delete_history(
    State(pool): State<SqlitePool>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    
    if let Some(confirm) = params.get("confirm") {
        if confirm == "true" {
            let result = sqlx::query("DELETE FROM detections")
                .execute(&pool)
                .await;
                
            match result {
                Ok(query_result) => {
                    let deleted_count = query_result.rows_affected();
                    println!("Historique supprimé: {} entrées", deleted_count);
                    
                    let notification = serde_json::json!({
                        "type": "history_cleared",
                        "data": {
                            "deleted_count": deleted_count
                        },
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    });
                    
                    notify_clients(&notification.to_string());
                    
                    Ok(ResponseJson(serde_json::json!({
                        "success": true,
                        "message": format!("{} détections supprimées", deleted_count)
                    })))
                }
                Err(e) => {
                    eprintln!("Erreur suppression: {}", e);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        } else {
            Ok(ResponseJson(serde_json::json!({
                "success": false,
                "message": "Confirmation requise"
            })))
        }
    } else {
        Ok(ResponseJson(serde_json::json!({
            "success": false,
            "message": "Paramètre 'confirm=true' requis"
        })))
    }
}

pub async fn get_color_mappings(State(pool): State<SqlitePool>) -> Result<ResponseJson<Vec<ColorMapping>>, StatusCode> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT g_id, type_name, color FROM types WHERE color IS NOT NULL ORDER BY g_id"
    )
    .fetch_all(&pool)
    .await;

    match rows {
        Ok(types_data) => {
            let mappings: Vec<ColorMapping> = types_data.into_iter()
                .filter_map(|(g_id, type_name, color)| {
                    color.map(|c| ColorMapping {
                        color: capitalize_color(&c),
                        g_id,
                        type_name,
                    })
                })
                .collect();
            
            println!("Mappings récupérés: {} entrées", mappings.len());
            Ok(ResponseJson(mappings))
        }
        Err(e) => {
            eprintln!("Erreur récupération mappings: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_types(State(pool): State<SqlitePool>) -> Result<ResponseJson<Vec<TypeInfo>>, StatusCode> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT g_id, type_name, color FROM types ORDER BY g_id"
    )
    .fetch_all(&pool)
    .await;

    match rows {
        Ok(types_data) => {
            let type_infos: Vec<TypeInfo> = types_data.into_iter().map(|(g_id, type_name, color)| {
                TypeInfo {
                    g_id,
                    type_name,
                    color,
                }
            }).collect();
            
            println!("Types récupérés: {} entrées", type_infos.len());
            Ok(ResponseJson(type_infos))
        }
        Err(e) => {
            eprintln!("Erreur récupération types: {}", e);
            let default_types = vec![
                TypeInfo {
                    g_id: "1001".to_string(),
                    type_name: "Carte microchip".to_string(),
                    color: Some("rouge".to_string()),
                },
                TypeInfo {
                    g_id: "1002".to_string(),
                    type_name: "Carte personnalisée".to_string(),
                    color: Some("vert".to_string()),
                },
                TypeInfo {
                    g_id: "1003".to_string(),
                    type_name: "STM32".to_string(),
                    color: Some("bleu".to_string()),
                },
                TypeInfo {
                    g_id: "1004".to_string(),
                    type_name: "Composant jaune".to_string(),
                    color: Some("jaune".to_string()),
                },
                TypeInfo {
                    g_id: "1005".to_string(),
                    type_name: "Composant noir".to_string(),
                    color: Some("noir".to_string()),
                },
            ];
            Ok(ResponseJson(default_types))
        }
    }
}

pub async fn add_type(
    State(pool): State<SqlitePool>,
    Json(payload): Json<AddTypeRequest>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    
    let existing = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM types WHERE g_id = ?1 OR type_name = ?2"
    )
    .bind(&payload.g_id)
    .bind(&payload.type_name)
    .fetch_one(&pool)
    .await;

    match existing {
        Ok(count) => {
            if count > 0 {
                return Ok(ResponseJson(serde_json::json!({
                    "success": false,
                    "message": "Type ou G_ID déjà existant"
                })));
            }
        }
        Err(e) => {
            eprintln!("Erreur vérification type: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    let result = sqlx::query("INSERT INTO types (g_id, type_name, color) VALUES (?1, ?2, ?3)")
        .bind(&payload.g_id)
        .bind(&payload.type_name)
        .bind(&payload.color)
        .execute(&pool)
        .await;

    match result {
        Ok(_) => {
            println!("Nouveau type ajouté: {} - {}", payload.g_id, payload.type_name);
            
            let notification = serde_json::json!({
                "type": "type_added",
                "data": {
                    "g_id": payload.g_id,
                    "type_name": payload.type_name,
                    "color": payload.color
                },
                "timestamp": chrono::Utc::now().to_rfc3339()
            });
            
            notify_clients(&notification.to_string());
            
            Ok(ResponseJson(serde_json::json!({
                "success": true,
                "message": "Type ajouté avec succès"
            })))
        }
        Err(e) => {
            eprintln!("Erreur ajout type: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn update_cadence(
    State(pool): State<SqlitePool>,
    Json(payload): Json<CadenceUpdate>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    
    let result = sqlx::query(
        "INSERT OR REPLACE INTO cadence (g_id, cadence_value, last_update) VALUES (?1, ?2, datetime('now', 'localtime'))"
    )
    .bind(&payload.g_id)
    .bind(payload.cadence)
    .execute(&pool)
    .await;

    match result {
        Ok(_) => {
            Ok(ResponseJson(serde_json::json!({
                "success": true,
                "g_id": payload.g_id,
                "cadence": payload.cadence
            })))
        }
        Err(e) => {
            eprintln!("Erreur mise à jour cadence: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_stats(
    State(pool): State<SqlitePool>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    match database::get_detection_stats(&pool).await {
        Ok(stats) => Ok(ResponseJson(stats)),
        Err(e) => {
            eprintln!("Erreur récupération stats: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn cleanup_detections(
    State(pool): State<SqlitePool>,
    Json(payload): Json<serde_json::Value>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    let days_to_keep = payload.get("days").and_then(|d| d.as_i64()).unwrap_or(30) as i32;
    
    match database::cleanup_old_detections(&pool, days_to_keep).await {
        Ok(deleted_count) => {
            let notification = serde_json::json!({
                "type": "cleanup_completed",
                "data": {
                    "deleted_count": deleted_count,
                    "days_kept": days_to_keep
                },
                "timestamp": chrono::Utc::now().to_rfc3339()
            });
            
            notify_clients(&notification.to_string());
            
            Ok(ResponseJson(serde_json::json!({
                "success": true,
                "message": format!("{} anciennes détections supprimées", deleted_count),
                "deleted_count": deleted_count
            })))
        },
        Err(e) => {
            eprintln!("Erreur nettoyage: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn create_backup(
    Json(payload): Json<serde_json::Value>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    let backup_name = payload.get("name").and_then(|n| n.as_str()).unwrap_or("backup");
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let backup_path = format!("../backups/{}_{}.db", backup_name, timestamp);
    
    match database::backup_database(&backup_path).await {
        Ok(_) => {
            println!("Sauvegarde créée: {}", backup_path);
            
            let notification = serde_json::json!({
                "type": "backup_created",
                "data": {
                    "backup_path": backup_path,
                    "backup_name": backup_name
                },
                "timestamp": chrono::Utc::now().to_rfc3339()
            });
            
            notify_clients(&notification.to_string());
            
            Ok(ResponseJson(serde_json::json!({
                "success": true,
                "message": "Sauvegarde créée avec succès",
                "backup_path": backup_path
            })))
        },
        Err(e) => {
            eprintln!("Erreur sauvegarde: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_recent_detections_handler(
    State(pool): State<SqlitePool>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<ResponseJson<serde_json::Value>, StatusCode> {
    let limit: i64 = params
        .get("limit")
        .and_then(|l| l.parse().ok())
        .unwrap_or(10);

    let hours: i64 = params
        .get("hours")
        .and_then(|h| h.parse().ok())
        .unwrap_or(24);

    println!("Récupération des {} détections des {} dernières heures", limit, hours);

    let detections_result = sqlx::query_as::<_, (i64, String, String, String, String, String, f64, f64, f64)>(
        "SELECT id, g_id, type_name, color, date_time, image_path, centroid_x, centroid_y, confidence
         FROM detections 
         WHERE date_time >= datetime('now', '-' || ?1 || ' hours')
         ORDER BY date_time DESC 
         LIMIT ?2"
    )
    .bind(hours)
    .bind(limit)
    .fetch_all(&pool)
    .await;

    match detections_result {
        Ok(rows) => {
            let detections: Vec<Detection> = rows
                .into_iter()
                .map(|(id, g_id, type_name, color, date_time, image_path, centroid_x, centroid_y, confidence)| Detection {
                    id,
                    g_id,
                    type_name,
                    color,
                    date_time: Some(date_time),
                    image_path: format!("/assets/captures/{}", image_path),
                    centroid_x: Some(centroid_x),
                    centroid_y: Some(centroid_y),
                    confidence: Some(confidence),
                })
                .collect();

            Ok(ResponseJson(serde_json::json!({
                "success": true,
                "detections": detections,
                "count": detections.len(),
                "period_hours": hours,
                "timestamp": chrono::Utc::now().to_rfc3339()
            })))
        }
        Err(e) => {
            eprintln!("Erreur récupération détections récentes: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// === HANDLERS WEBSOCKET ===

pub async fn websocket_handler(ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(handle_websocket)
}

async fn handle_websocket(socket: WebSocket) {
    println!("Nouvelle connexion WebSocket");
    
    unsafe {
        if let Some(sender) = &NOTIFICATION_SENDER {
            let mut rx = sender.subscribe();
            let (mut ws_tx, mut ws_rx) = socket.split();
            
            let welcome = serde_json::json!({
                "type": "connected",
                "message": "WebSocket ESP32 connecté",
                "server": "rust-axum-esp32-v2",
                "timestamp": chrono::Utc::now().to_rfc3339()
            });
            
            if ws_tx.send(axum::extract::ws::Message::Text(welcome.to_string())).await.is_err() {
                println!("Erreur envoi message de bienvenue");
                return;
            }
            
            println!("Client WebSocket connecté");
            
            tokio::select! {
                _ = async {
                    while let Ok(msg) = rx.recv().await {
                        println!("Envoi notification WebSocket: {}", msg);
                        
                        if ws_tx.send(axum::extract::ws::Message::Text(msg)).await.is_err() {
                            println!("Connexion WebSocket fermée");
                            break;
                        }
                    }
                } => {}
                _ = async {
                    while let Some(msg) = ws_rx.next().await {
                        if msg.is_err() {
                            println!("Erreur réception message WebSocket");
                            break;
                        }
                        if let Ok(axum::extract::ws::Message::Text(text)) = msg {
                            println!("Message reçu du client: {}", text);
                        }
                    }
                } => {}
            }
            
            println!("Connexion WebSocket fermée");
        }
    }
}

// === FONCTIONS UTILITAIRES ===

fn validate_esp32_input(payload: &ESP32UpdateRequest) -> Vec<String> {
    let mut errors = Vec::new();

    if payload.g_id.len() != 4 || !payload.g_id.chars().all(|c| c.is_ascii_digit()) {
        errors.push("G_ID doit contenir exactement 4 chiffres".to_string());
    }

    let valid_colors = vec!["vert", "rouge", "bleu", "jaune", "noir"];
    if !valid_colors.contains(&payload.color.to_lowercase().as_str()) {
        errors.push(format!("Couleur '{}' non supportée. Couleurs valides: {}", 
                           payload.color, valid_colors.join(", ")));
    }

    if payload.type_name.is_empty() {
        errors.push("Le nom ne peut pas être vide".to_string());
    } else if !payload.type_name.chars().all(|c| {
        c.is_ascii_digit() || matches!(c.to_ascii_uppercase(), 'A'..='D')
    }) {
        errors.push("Nom doit contenir uniquement A-D et 0-9".to_string());
    }

    if payload.type_name.len() > 8 {
        errors.push("Nom trop long (max 8 caractères)".to_string());
    }

    errors
}

fn normalize_color_name(color: &str) -> String {
    match color.to_lowercase().as_str() {
        "red" | "rouge" => "rouge".to_string(),
        "green" | "vert" => "vert".to_string(),
        "blue" | "bleu" => "bleu".to_string(),
        "yellow" | "jaune" => "jaune".to_string(),
        "black" | "noir" => "noir".to_string(),
        _ => color.to_lowercase(),
    }
}

fn capitalize_color(color: &str) -> String {
    match color.to_lowercase().as_str() {
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

async fn save_image_from_base64(data_url: &str, color: &str) -> Result<String, Box<dyn std::error::Error>> {
    if !data_url.starts_with("data:image/") {
        return Err("Format d'image invalide".into());
    }

    let parts: Vec<&str> = data_url.splitn(2, ',').collect();
    if parts.len() != 2 {
        return Err("Format data URL invalide".into());
    }

    let header = parts[0];
    let data = parts[1];

    let extension = if header.contains("jpeg") || header.contains("jpg") {
        "jpg"
    } else if header.contains("png") {
        "png"
    } else if header.contains("webp") {
        "webp"
    } else {
        "jpg"
    };

    let image_data = general_purpose::STANDARD.decode(data)?;

    let filename = format!("esp32_{}_{}.{}", 
    chrono::Utc::now().format("%Y%m%d_%H%M%S"),
    color,  // Utiliser la couleur au lieu de UUID
    extension
);

    fs::create_dir_all("../frontend/assets/captures")?;

    let filepath = format!("../frontend/assets/captures/{}", filename);
    fs::write(&filepath, image_data)?;

    println!("Image ESP32 sauvegardée: {}", filename);
    Ok(filename)
}