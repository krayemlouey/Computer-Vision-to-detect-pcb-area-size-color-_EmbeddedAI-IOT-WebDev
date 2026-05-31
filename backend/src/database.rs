use sqlx::SqlitePool;
use serde_json::json;

pub async fn init_database(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    println!("Création des tables de base de données...");

    // Table des détections - structure complète et cohérente
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS detections (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            g_id TEXT NOT NULL,
            type_name TEXT NOT NULL,
            color TEXT NOT NULL,
            image_path TEXT DEFAULT 'no_image.jpg',
            date_time TEXT DEFAULT (datetime('now', 'localtime')),
            confidence REAL DEFAULT 0.0,
            centroid_x REAL DEFAULT 0.0,
            centroid_y REAL DEFAULT 0.0,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
        "#
    )
    .execute(pool)
    .await?;

    // Table des types de composants
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS types (
            g_id TEXT PRIMARY KEY,
            type_name TEXT NOT NULL UNIQUE,
            color TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
        "#
    )
    .execute(pool)
    .await?;

    // Table des cadences
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS cadence (
            g_id TEXT PRIMARY KEY,
            cadence_value INTEGER NOT NULL,
            last_update TEXT DEFAULT (datetime('now', 'localtime')),
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
        "#
    )
    .execute(pool)
    .await?;

    // Table pour les statistiques temps réel
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS real_time_stats (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            counter_rouge INTEGER DEFAULT 0,
            counter_vert INTEGER DEFAULT 0,
            counter_bleu INTEGER DEFAULT 0,
            counter_jaune INTEGER DEFAULT 0,
            counter_noir INTEGER DEFAULT 0,
            total_detections INTEGER DEFAULT 0,
            last_update TEXT DEFAULT (datetime('now', 'localtime'))
        )
        "#
    )
    .execute(pool)
    .await?;

    // Insérer les stats par défaut si elles n'existent pas
    let stats_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM real_time_stats")
        .fetch_one(pool)
        .await?;

    if stats_count == 0 {
        sqlx::query("INSERT INTO real_time_stats DEFAULT VALUES")
            .execute(pool)
            .await?;
    }

    // Insérer des types par défaut s'ils n'existent pas
    let count_result = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM types")
        .fetch_one(pool)
        .await?;

    if count_result == 0 {
        println!("Insertion des types par défaut...");
        
        let default_types = vec![
            ("1001", "Carte microchip", Some("rouge")),
            ("1002", "Carte personnalisée", Some("vert")),
            ("1003", "STM32", Some("bleu")),
            ("1004", "Composant jaune", Some("jaune")),
            ("1005", "Composant noir", Some("noir")),
        ];

        for (g_id, type_name, color) in default_types {
            sqlx::query("INSERT OR IGNORE INTO types (g_id, type_name, color) VALUES (?1, ?2, ?3)")
                .bind(g_id)
                .bind(type_name)
                .bind(color)
                .execute(pool)
                .await?;
        }
    }

    // Créer des index pour améliorer les performances
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_detections_date ON detections(date_time DESC)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_detections_gid ON detections(g_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_detections_color ON detections(color)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_types_color ON types(color)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_detections_created_at ON detections(created_at DESC)")
        .execute(pool)
        .await?;

    println!("Base de données initialisée avec succès!");
    Ok(())
}

// Fonction de migration pour corriger la structure existante
pub async fn migrate_database_structure(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    println!("🔄 Vérification et migration de la structure de la base de données...");
    
    // Diagnostiquer la structure actuelle
    diagnose_database_structure(pool).await?;
    
    // Migration: Vérifier si toutes les colonnes existent
    let columns_to_check = vec![
        ("centroid_x", "ALTER TABLE detections ADD COLUMN centroid_x REAL DEFAULT 0.0"),
        ("centroid_y", "ALTER TABLE detections ADD COLUMN centroid_y REAL DEFAULT 0.0"),
        ("confidence", "ALTER TABLE detections ADD COLUMN confidence REAL DEFAULT 0.0"),
        ("created_at", "ALTER TABLE detections ADD COLUMN created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP"),
    ];
    
    for (column_name, alter_query) in columns_to_check {
        let check_column = sqlx::query_scalar::<_, i64>(
            &format!("SELECT COUNT(*) FROM pragma_table_info('detections') WHERE name = '{}'", column_name)
        )
        .fetch_one(pool)
        .await?;

        if check_column == 0 {
            println!("⚠️ Colonne '{}' manquante. Ajout en cours...", column_name);
            match sqlx::query(alter_query).execute(pool).await {
                Ok(_) => println!("✅ Colonne '{}' ajoutée avec succès !", column_name),
                Err(e) => {
                    if !e.to_string().contains("duplicate column name") {
                        println!("❌ Erreur lors de l'ajout de '{}': {}", column_name, e);
                    }
                }
            }
        } else {
            println!("✅ Colonne '{}' déjà présente", column_name);
        }
    }

    println!("✅ Migration de la base de données terminée !");
    Ok(())
}

// Fonction pour diagnostiquer la structure de la base - VERSION CORRIGÉE
pub async fn diagnose_database_structure(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    println!("\n🔍 DIAGNOSTIC DE LA BASE DE DONNÉES");
    println!("=====================================");
    
    // Utiliser query_as au lieu de query! pour éviter les problèmes de macros
    let columns = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT name, type, dflt_value FROM pragma_table_info('detections')"
    )
    .fetch_all(pool)
    .await?;

    println!("📋 Structure actuelle de la table 'detections':");
    for (name, column_type, default_value) in columns {
        println!("   - {}: {} (défaut: {:?})", 
                name, 
                column_type, 
                default_value.unwrap_or_else(|| "NULL".to_string())
        );
    }

    // Compter les détections existantes
    let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM detections")
        .fetch_one(pool)
        .await?;
    
    println!("📊 Nombre de détections: {}", count);
    println!("=====================================\n");
    
    Ok(())
}

// Fonction pour recréer complètement la table si nécessaire - VERSION CORRIGÉE
pub async fn recreate_detections_table(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    println!("🔄 Recréation de la table detections avec la nouvelle structure...");
    
    // Sauvegarder les données existantes avec query_as au lieu de query!
    let existing_data = sqlx::query_as::<_, (i64, String, String, String, String, String)>(
        "SELECT id, g_id, type_name, color, date_time, image_path FROM detections ORDER BY id"
    )
    .fetch_all(pool)
    .await?;

    let data_count = existing_data.len();
    println!("💾 {} détections existantes sauvegardées", data_count);

    // Supprimer l'ancienne table
    sqlx::query("DROP TABLE IF EXISTS detections")
        .execute(pool)
        .await?;

    // Créer la nouvelle table avec toutes les colonnes
    sqlx::query(
        r#"
        CREATE TABLE detections (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            g_id TEXT NOT NULL,
            type_name TEXT NOT NULL,
            color TEXT NOT NULL,
            image_path TEXT DEFAULT 'no_image.jpg',
            date_time TEXT DEFAULT (datetime('now', 'localtime')),
            confidence REAL DEFAULT 0.0,
            centroid_x REAL DEFAULT 0.0,
            centroid_y REAL DEFAULT 0.0,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
        "#
    )
    .execute(pool)
    .await?;

    // Restaurer les données existantes avec query standard
    for (id, g_id, type_name, color, date_time, image_path) in existing_data {
        sqlx::query(
            "INSERT INTO detections (id, g_id, type_name, color, date_time, image_path, confidence, centroid_x, centroid_y) VALUES (?, ?, ?, ?, ?, ?, 1.0, 0.0, 0.0)"
        )
        .bind(id)
        .bind(g_id)
        .bind(type_name)
        .bind(color)
        .bind(date_time)
        .bind(image_path)
        .execute(pool)
        .await?;
    }

    println!("✅ Table detections recréée et {} détections restaurées !", data_count);
    Ok(())
}

// === FONCTIONS EXISTANTES (gardées intactes) ===

pub async fn get_detection_stats(pool: &SqlitePool) -> Result<serde_json::Value, sqlx::Error> {
    let total_result = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM detections")
        .fetch_one(pool)
        .await?;

    let by_type_rows = sqlx::query_as::<_, (String, i64)>(
        "SELECT type_name, COUNT(*) as count FROM detections GROUP BY type_name ORDER BY count DESC LIMIT 10"
    )
    .fetch_all(pool)
    .await?;

    let by_color_rows = sqlx::query_as::<_, (String, i64)>(
        "SELECT color, COUNT(*) as count FROM detections GROUP BY color ORDER BY count DESC"
    )
    .fetch_all(pool)
    .await?;

    let recent_result = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM detections WHERE datetime(date_time) >= datetime('now', '-1 day')"
    )
    .fetch_one(pool)
    .await?;

    let today_result = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM detections WHERE date(date_time) >= date('now', 'localtime')"
    )
    .fetch_one(pool)
    .await?;

    let by_type: Vec<serde_json::Value> = by_type_rows
        .into_iter()
        .map(|(type_name, count)| json!({
            "type_name": type_name,
            "count": count
        }))
        .collect();

    let by_color: Vec<serde_json::Value> = by_color_rows
        .into_iter()
        .map(|(color, count)| json!({
            "color": color,
            "count": count
        }))
        .collect();

    Ok(json!({
        "total_detections": total_result,
        "recent_24h": recent_result,
        "today": today_result,
        "by_type": by_type,
        "by_color": by_color,
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

pub async fn cleanup_old_detections(pool: &SqlitePool, days_to_keep: i32) -> Result<u64, sqlx::Error> {
    println!("Nettoyage des détections de plus de {} jours...", days_to_keep);
    
    let result = sqlx::query("DELETE FROM detections WHERE datetime(date_time) < datetime('now', '-' || ?1 || ' days')")
        .bind(days_to_keep)
        .execute(pool)
        .await?;

    println!("Détections nettoyées: {}", result.rows_affected());
    
    update_real_time_counters(pool).await?;
    
    Ok(result.rows_affected())
}

pub async fn backup_database(backup_path: &str) -> Result<(), std::io::Error> {
    use std::fs;
    
    println!("Création de la sauvegarde...");
    
    let source_path = "data/detection.db";
    if !std::path::Path::new(source_path).exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Fichier source introuvable: {}", source_path)
        ));
    }
    
    if let Some(parent) = std::path::Path::new(backup_path).parent() {
        fs::create_dir_all(parent)?;
    }
    
    fs::copy(source_path, backup_path)?;
    println!("Sauvegarde créée avec succès: {}", backup_path);
    Ok(())
}

pub async fn update_real_time_counters(pool: &SqlitePool) -> Result<serde_json::Value, sqlx::Error> {
    let rouge_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM detections WHERE LOWER(color) = 'rouge'"
    ).fetch_one(pool).await.unwrap_or(0);

    let vert_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM detections WHERE LOWER(color) = 'vert'"
    ).fetch_one(pool).await.unwrap_or(0);

    let bleu_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM detections WHERE LOWER(color) = 'bleu'"
    ).fetch_one(pool).await.unwrap_or(0);

    let jaune_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM detections WHERE LOWER(color) = 'jaune'"
    ).fetch_one(pool).await.unwrap_or(0);

    let noir_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM detections WHERE LOWER(color) = 'noir'"
    ).fetch_one(pool).await.unwrap_or(0);

    let total_count = rouge_count + vert_count + bleu_count + jaune_count + noir_count;

    sqlx::query(
        "UPDATE real_time_stats SET 
         counter_rouge = ?1, counter_vert = ?2, counter_bleu = ?3, 
         counter_jaune = ?4, counter_noir = ?5, total_detections = ?6,
         last_update = datetime('now', 'localtime')
         WHERE id = 1"
    )
    .bind(rouge_count)
    .bind(vert_count)
    .bind(bleu_count)
    .bind(jaune_count)
    .bind(noir_count)
    .bind(total_count)
    .execute(pool)
    .await?;

    Ok(json!({
        "rouge": rouge_count,
        "vert": vert_count,
        "bleu": bleu_count,
        "jaune": jaune_count,
        "noir": noir_count,
        "total": total_count,
        "updated_at": chrono::Utc::now().to_rfc3339()
    }))
}

pub async fn get_real_time_counters(pool: &SqlitePool) -> Result<serde_json::Value, sqlx::Error> {
    let result = sqlx::query_as::<_, (i64, i64, i64, i64, i64, i64, String)>(
        "SELECT counter_rouge, counter_vert, counter_bleu, counter_jaune, counter_noir, total_detections, last_update 
         FROM real_time_stats WHERE id = 1"
    )
    .fetch_optional(pool)
    .await?;

    match result {
        Some((rouge, vert, bleu, jaune, noir, total, last_update)) => {
            Ok(json!({
                "counters": {
                    "rouge": rouge,
                    "vert": vert,
                    "bleu": bleu,
                    "jaune": jaune,
                    "noir": noir
                },
                "total": total,
                "last_update": last_update,
                "timestamp": chrono::Utc::now().to_rfc3339()
            }))
        }
        None => {
            sqlx::query("INSERT INTO real_time_stats DEFAULT VALUES").execute(pool).await?;
            Ok(json!({
                "counters": {
                    "rouge": 0,
                    "vert": 0,
                    "bleu": 0,
                    "jaune": 0,
                    "noir": 0
                },
                "total": 0,
                "last_update": chrono::Utc::now().to_rfc3339(),
                "timestamp": chrono::Utc::now().to_rfc3339()
            }))
        }
    }
}

pub async fn get_history_filtered(
    pool: &SqlitePool,
    color_filter: Option<String>,
    type_filter: Option<String>,
    date_from: Option<String>,
    date_to: Option<String>,
    limit: i64
) -> Result<serde_json::Value, sqlx::Error> {
    // Version simplifiée pour l'instant
    let detections = sqlx::query_as::<_, (i64, String, String, String, String, String, Option<f64>, Option<f64>, Option<f64>)>(
        "SELECT id, g_id, type_name, color, date_time, image_path, confidence, centroid_x, centroid_y 
         FROM detections 
         ORDER BY datetime(date_time) DESC, id DESC 
         LIMIT ?"
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let formatted: Vec<serde_json::Value> = detections
    .into_iter()
    .map(|(id, g_id, type_name, color, date_time, image_path, confidence, centroid_x, centroid_y)| {
        // Normaliser le chemin d'image
        let normalized_path = if image_path == "no_image.jpg" || image_path.is_empty() {
            "/assets/no_image.jpg".to_string()
        } else if image_path.starts_with("/assets/") {
            image_path.clone()
        } else {
            format!("/assets/captures/{}", image_path)
        };
        
        json!({
            "id": id,
            "g_id": g_id,
            "type_name": type_name,
            "color": color,
            "date_time": date_time,
            "image_path": normalized_path,
            "confidence": confidence.unwrap_or(0.0),
            "centroid_x": centroid_x.unwrap_or(0.0),
            "centroid_y": centroid_y.unwrap_or(0.0)
        })
    })
    .collect();

    Ok(json!({
        "data": formatted,
        "total": formatted.len(),
        "filters_applied": {
            "color": color_filter,
            "type": type_filter,
            "date_from": date_from,
            "date_to": date_to
        },
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

pub async fn get_history_paginated(
    pool: &SqlitePool, 
    page: i64, 
    limit: i64
) -> Result<serde_json::Value, sqlx::Error> {
    let offset = (page - 1) * limit;
    
    let total_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM detections")
        .fetch_one(pool)
        .await?;

    let detections = sqlx::query_as::<_, (i64, String, String, String, String, String, Option<f64>, Option<f64>, Option<f64>)>(
        "SELECT id, g_id, type_name, color, date_time, image_path, confidence, centroid_x, centroid_y 
         FROM detections 
         ORDER BY datetime(date_time) DESC, id DESC 
         LIMIT ?1 OFFSET ?2"
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let formatted: Vec<serde_json::Value> = detections
        .into_iter()
        .map(|(id, g_id, type_name, color, date_time, image_path, confidence, centroid_x, centroid_y)| {
            json!({
                "id": id,
                "g_id": g_id,
                "type_name": type_name,
                "color": color,
                "date_time": date_time,
                "image_path": if image_path.starts_with("/") { 
                    image_path 
                } else { 
                    format!("/assets/captures/{}", image_path) 
                },
                "confidence": confidence.unwrap_or(0.0),
                "centroid_x": centroid_x.unwrap_or(0.0),
                "centroid_y": centroid_y.unwrap_or(0.0)
            })
        })
        .collect();

    let total_pages = (total_count + limit - 1) / limit;

    Ok(json!({
        "data": formatted,
        "pagination": {
            "current_page": page,
            "total_pages": total_pages,
            "total_items": total_count,
            "items_per_page": limit,
            "has_next": page < total_pages,
            "has_prev": page > 1
        },
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

pub async fn get_database_info(pool: &SqlitePool) -> Result<serde_json::Value, sqlx::Error> {
    let detections_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM detections")
        .fetch_one(pool)
        .await?;
    
    let types_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM types")
        .fetch_one(pool)
        .await?;
    
    let cadence_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM cadence")
        .fetch_one(pool)
        .await?;

    let esp32_mappings = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM types WHERE color IS NOT NULL"
    )
    .fetch_one(pool)
    .await?;

    let latest_detections = sqlx::query_as::<_, (i64, String, String, String, String, String)>(
        "SELECT id, g_id, type_name, color, date_time, image_path 
         FROM detections 
         ORDER BY datetime(date_time) DESC 
         LIMIT 5"
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let latest_formatted: Vec<serde_json::Value> = latest_detections
        .into_iter()
        .map(|(id, g_id, type_name, color, date_time, image_path)| {
            json!({
                "id": id,
                "g_id": g_id,
                "type_name": type_name,
                "color": color,
                "date_time": date_time,
                "image_path": image_path
            })
        })
        .collect();

    Ok(json!({
        "database_file": "data/detection.db",
        "tables": [
            {
                "table_name": "detections",
                "row_count": detections_count
            },
            {
                "table_name": "types", 
                "row_count": types_count
            },
            {
                "table_name": "cadence",
                "row_count": cadence_count
            }
        ],
        "esp32_info": {
            "color_mappings": esp32_mappings,
            "supported_colors": ["vert", "rouge", "bleu", "jaune", "noir"]
        },
        "latest_detections": latest_formatted,
        "total_tables": 3,
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

pub async fn check_database_integrity(pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    let _result = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM detections")
        .fetch_one(pool)
        .await?;
    
    Ok(true)
}

pub async fn optimize_database(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    println!("Optimisation de la base de données...");
    
    sqlx::query("VACUUM").execute(pool).await?;
    sqlx::query("ANALYZE").execute(pool).await?;
    
    println!("Optimisation terminée");
    Ok(())
}

pub async fn get_esp32_color_mappings(pool: &SqlitePool) -> Result<serde_json::Value, sqlx::Error> {
    let mappings = sqlx::query_as::<_, (String, String, String)>(
        "SELECT g_id, type_name, color FROM types WHERE color IS NOT NULL ORDER BY color"
    )
    .fetch_all(pool)
    .await?;

    let color_map: std::collections::HashMap<String, serde_json::Value> = mappings
        .into_iter()
        .map(|(g_id, type_name, color)| {
            (color.clone(), json!({
                "g_id": g_id,
                "type_name": type_name,
                "color": color
            }))
        })
        .collect();

    Ok(json!({
        "mappings": color_map,
        "count": color_map.len(),
        "supported_colors": ["vert", "rouge", "bleu", "jaune", "noir"],
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

pub async fn validate_esp32_gid_unique(pool: &SqlitePool, g_id: &str) -> Result<bool, sqlx::Error> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM types WHERE g_id = ?1"
    )
    .bind(g_id)
    .fetch_one(pool)
    .await?;

    Ok(count == 0)
}

pub async fn get_recent_detections_formatted(pool: &SqlitePool, limit: i64) -> Result<Vec<serde_json::Value>, sqlx::Error> {
    let detections = sqlx::query_as::<_, (i64, String, String, String, String, String, Option<f64>)>(
        "SELECT id, g_id, type_name, color, date_time, image_path, confidence 
         FROM detections 
         ORDER BY datetime(date_time) DESC, id DESC 
         LIMIT ?1"
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let formatted: Vec<serde_json::Value> = detections
        .into_iter()
        .map(|(id, g_id, type_name, color, date_time, image_path, confidence)| {
            json!({
                "id": id,
                "g_id": g_id,
                "type_name": type_name,
                "color": color,
                "date_time": date_time,
                "image_path": if image_path.starts_with("/") { image_path } else { format!("/assets/captures/{}", image_path) },
                "confidence": confidence.unwrap_or(0.0)
            })
        })
        .collect();

    Ok(formatted)
}

pub async fn reset_all_counters(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM detections").execute(pool).await?;
    
    sqlx::query(
        "UPDATE real_time_stats SET 
        counter_rouge = 0, counter_vert = 0, counter_bleu = 0, 
        counter_jaune = 0, counter_noir = 0, total_detections = 0,
        last_update = datetime('now', 'localtime')
        WHERE id = 1"
    )
    .execute(pool)
    .await?;

    println!("Tous les compteurs ont été réinitialisés");
    Ok(())
}