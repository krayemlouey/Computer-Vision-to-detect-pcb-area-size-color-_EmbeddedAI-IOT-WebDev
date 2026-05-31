/*
 * ============================================================
 *  ESP32 - Système de Détection de Couleur
 *  Keypad 4x4 + LCD I2C + WebServer + HTTP vers Rust Backend
 *
 *  Bibliothèques requises (Arduino Library Manager):
 *    - Keypad             (Mark Stanley)
 *    - LiquidCrystal_I2C  (marcoschwartz) ← IMPORTANT: ESP32-compatible version, NOT Frank de Brabander
 *    - ArduinoJson        (Benoit Blanchon) v6.x
 *    - HTTPClient         (inclus avec ESP32 core)
 * ============================================================
 *
 *  CONFIGURATION KEYPAD 4x4 :
 *    Lignes  : GPIO 19, 18, 5, 17
 *    Colonnes: GPIO 16,  4, 2, 15
 *
 *  CONFIGURATION LCD I2C :
 *    SDA: GPIO 21
 *    SCL: GPIO 22
 *    Adresse I2C: 0x27 (vérifiez avec un scanner I2C)
 *
 *  FLUX UTILISATION :
 *    1. Appuyer * → Sélectionner couleur (1-5)
 *    2. Saisir G_ID (4 chiffres, # efface)
 *    3. Saisir Nom (A-D + 0-9, max 8 chars, * pour valider)
 *    4. Confirmer avec # ou annuler avec *
 *    5. Envoi automatique vers le serveur Rust
 * ============================================================
 */

#include <WiFi.h>
#include <WebServer.h>
#include <HTTPClient.h>
#include <ArduinoJson.h>
#include <Keypad.h>
#include <LiquidCrystal_I2C.h>
#include <Wire.h>

// ============================================================
//  CONFIGURATION - MODIFIEZ CES VALEURS
// ============================================================
const char* WIFI_SSID     = "louey";
const char* WIFI_PASSWORD = "123456789";
const char* SERVER_IP     = "172.20.158.34";  // IP du serveur Rust
const int   SERVER_PORT   = 3001;

// ============================================================
//  CONFIGURATION HARDWARE
// ============================================================

// Keypad 4x4
const byte ROWS = 4;
const byte COLS = 4;
char keys[ROWS][COLS] = {
  {'1','2','3','A'},
  {'4','5','6','B'},
  {'7','8','9','C'},
  {'*','0','#','D'}
};
byte rowPins[ROWS] = {19, 18, 5, 17};
byte colPins[COLS] = {16, 4, 2, 15};
Keypad keypad = Keypad(makeKeymap(keys), rowPins, colPins, ROWS, COLS);

// LCD I2C 16x2
LiquidCrystal_I2C lcd(0x27, 16, 2);

// Serveur web embarqué ESP32 (port 80)
WebServer server(80);

// ============================================================
//  MACHINE À ÉTATS
// ============================================================
enum SystemState {
  STATE_IDLE,
  STATE_SELECT_COLOR,
  STATE_ENTER_GID,
  STATE_ENTER_NAME,
  STATE_CONFIRM,
  STATE_SENDING
};

SystemState currentState = STATE_IDLE;

// ============================================================
//  VARIABLES GLOBALES
// ============================================================
String selectedColor  = "";  // Couleur en français (ex: "Vert")
String selectedGID    = "";  // G_ID 4 chiffres
String selectedName   = "";  // Nom type (A-D, 0-9, max 8)
String currentInput   = "";  // Saisie en cours

bool   serverReachable  = false;  // Serveur Rust accessible ?
unsigned long lastServerCheck = 0;
unsigned long lastIdleRefresh = 0;
bool   showIPMode = false;

// Mappings couleurs FR <-> EN
const char* COLOR_FR[] = {"", "Vert", "Rouge", "Bleu", "Jaune", "Noir"};
const char* COLOR_EN[] = {"", "vert", "rouge", "bleu", "jaune", "noir"};

// ============================================================
//  SETUP
// ============================================================
void setup() {
  Serial.begin(115200);
  Serial.println("\n========================================");
  Serial.println("  ESP32 - Systeme Detection Couleur");
  Serial.println("========================================");

  // LCD init
  Wire.begin(21, 22);
  lcd.init();
  lcd.backlight();
  lcdPrint("ESP32 Starting..", "Please wait...");
  delay(1500);

  // WiFi
  connectToWiFi();

  // Routes serveur web
  setupWebServer();

  // Test serveur Rust
  checkServerReachable();

  // Affichage initial
  showMainMenu();

  Serial.println("\n=== SYSTEME PRET ===");
  Serial.println("  * = Demarrer configuration");
  Serial.println("  D = Info reseau");
  Serial.printf("  ESP32 Web: http://%s/\n", WiFi.localIP().toString().c_str());
  Serial.printf("  Rust API:  http://%s:%d/\n", SERVER_IP, SERVER_PORT);
  Serial.println("========================================\n");
}

// ============================================================
//  LOOP
// ============================================================
void loop() {
  // Gestion web server (NON BLOQUANT - priorité absolue)
  server.handleClient();

  // Reconnexion WiFi automatique
  if (WiFi.status() != WL_CONNECTED) {
    Serial.println("⚠️  WiFi perdu - Reconnexion...");
    lcdPrint("WiFi Lost!", "Reconnecting...");
    connectToWiFi();
    return;
  }

  // Lecture keypad
  char key = keypad.getKey();
  if (key) {
    Serial.printf("\n[TOUCHE] '%c' | État: %s\n", key, getStateStr());
    handleKeyPress(key);
  }

  // Rafraîchissement idle toutes les 8 secondes (sans delay)
  if (currentState == STATE_IDLE && millis() - lastIdleRefresh > 8000) {
    showIPMode = !showIPMode;
    updateIdleDisplay();
    lastIdleRefresh = millis();
  }

  // Vérification serveur toutes les 30 secondes (sans delay)
  if (millis() - lastServerCheck > 30000) {
    checkServerReachable();
    lastServerCheck = millis();
    if (currentState == STATE_IDLE) updateIdleDisplay();
  }
}

// ============================================================
//  UTILITAIRES LCD
// ============================================================

// Affichage 2 lignes sur LCD (max 16 chars chacune)
void lcdPrint(String line1, String line2) {
  lcd.clear();
  lcd.setCursor(0, 0);
  lcd.print(line1.substring(0, 16));
  lcd.setCursor(0, 1);
  lcd.print(line2.substring(0, 16));
}

// Efface le reste d'une ligne sur le LCD
void lcdClearLine(int row) {
  lcd.setCursor(0, row);
  lcd.print("                ");  // 16 espaces
  lcd.setCursor(0, row);
}

// ============================================================
//  WIFI
// ============================================================
void connectToWiFi() {
  Serial.printf("WiFi: Connexion à '%s'...\n", WIFI_SSID);
  lcdPrint("WiFi connecting.", "");

  WiFi.mode(WIFI_STA);
  WiFi.begin(WIFI_SSID, WIFI_PASSWORD);

  int attempts = 0;
  while (WiFi.status() != WL_CONNECTED && attempts < 20) {
    delay(500);
    Serial.print(".");
    attempts++;
  }

  if (WiFi.status() == WL_CONNECTED) {
    Serial.println("\n✅ WiFi connecté!");
    Serial.printf("   IP ESP32: %s\n", WiFi.localIP().toString().c_str());
    lcdPrint("WiFi OK!", WiFi.localIP().toString());
    delay(2000);
  } else {
    Serial.println("\n❌ WiFi ECHEC - Vérifiez les identifiants");
    lcdPrint("WiFi FAILED!", "Check config");
    delay(3000);
  }
}

// ============================================================
//  VÉRIFICATION SERVEUR RUST (sans delay bloquant)
// ============================================================
void checkServerReachable() {
  if (WiFi.status() != WL_CONNECTED) {
    serverReachable = false;
    return;
  }

  HTTPClient http;
  String url = "http://" + String(SERVER_IP) + ":" + String(SERVER_PORT) + "/api/esp32/ping";
  http.begin(url);
  http.setTimeout(4000);  // timeout 4s
  int code = http.GET();
  http.end();

  serverReachable = (code == 200);
  Serial.printf("Serveur Rust: %s (HTTP %d)\n", serverReachable ? "ONLINE ✅" : "OFFLINE ❌", code);
}

// ============================================================
//  VÉRIFICATION G_ID SUR LE SERVEUR RUST (CRITIQUE - corrigé)
// ============================================================
bool checkGIDOnServer(String gid) {
  if (WiFi.status() != WL_CONNECTED || !serverReachable) {
    Serial.println("⚠️  Impossible de vérifier G_ID - serveur inaccessible");
    return true;  // on laisse passer si pas de réseau (le serveur refusera)
  }

  HTTPClient http;
  String url = "http://" + String(SERVER_IP) + ":" + String(SERVER_PORT) + "/api/esp32/validate-data";
  http.begin(url);
  http.addHeader("Content-Type", "application/json");
  http.setTimeout(5000);

  // On envoie les données minimales pour vérifier le G_ID
  StaticJsonDocument<256> doc;
  doc["color"] = "vert";       // valeur placeholder valide
  doc["g_id"]  = gid;
  doc["type_name"] = "A";      // valeur placeholder valide
  doc["source"] = "ESP32";
  String body;
  serializeJson(doc, body);

  int code = http.POST(body);

  if (code != 200) {
    http.end();
    Serial.printf("❌ Erreur vérification G_ID (HTTP %d)\n", code);
    return true;  // laisser continuer, le serveur validera
  }

  String response = http.getString();
  http.end();

  StaticJsonDocument<512> resp;
  DeserializationError err = deserializeJson(resp, response);
  if (err) {
    Serial.println("❌ JSON invalide dans réponse G_ID check");
    return true;
  }

  bool valid = resp["valid"] | true;
  // Si "G_ID déjà utilisé" est dans les erreurs → G_ID non disponible
  if (!valid) {
    JsonArray errors = resp["errors"].as<JsonArray>();
    for (JsonVariant e : errors) {
      String errMsg = e.as<String>();
      Serial.println("  Erreur serveur: " + errMsg);
      if (errMsg.indexOf("G_ID") >= 0 && errMsg.indexOf("utilis") >= 0) {
        return false;  // G_ID déjà pris
      }
    }
  }

  return true;  // G_ID disponible
}

// ============================================================
//  ENVOI DES DONNÉES AU SERVEUR RUST (CORRIGÉ - HTTPClient)
// ============================================================
bool sendDataToRust(String colorEN, String gid, String typeName) {
  if (WiFi.status() != WL_CONNECTED) {
    Serial.println("❌ WiFi déconnecté");
    return false;
  }

  HTTPClient http;
  String url = "http://" + String(SERVER_IP) + ":" + String(SERVER_PORT) + "/api/esp32/update-mapping";
  http.begin(url);
  http.addHeader("Content-Type", "application/json");
  http.setTimeout(8000);

  StaticJsonDocument<512> doc;
  doc["color"]     = colorEN;
  doc["g_id"]      = gid;
  doc["type_name"] = typeName;
  doc["source"]    = "ESP32";

  String body;
  serializeJson(doc, body);

  Serial.println("📤 POST vers: " + url);
  Serial.println("📄 Body: " + body);

  int code = http.POST(body);
  String response = http.getString();
  http.end();

  Serial.printf("📨 Réponse HTTP %d: %s\n", code, response.c_str());

  if (code == 200) {
    StaticJsonDocument<512> resp;
    deserializeJson(resp, response);
    bool success = resp["success"] | false;
    if (success) {
      Serial.println("✅ Mapping envoyé avec succès!");
      return true;
    } else {
      String msg = resp["message"] | "Erreur inconnue";
      Serial.println("❌ Refus du serveur: " + msg);
      return false;
    }
  }

  return false;
}

// ============================================================
//  NAVIGATION / AFFICHAGE ÉTATS
// ============================================================
void showMainMenu() {
  currentState  = STATE_IDLE;
  selectedColor = "";
  selectedGID   = "";
  selectedName  = "";
  currentInput  = "";
  showIPMode    = false;
  lastIdleRefresh = millis();
  updateIdleDisplay();
  Serial.println("\n[MENU] Prêt | * = Start | D = IP Info");
}

void updateIdleDisplay() {
  if (showIPMode) {
    lcdPrint("IP: " + WiFi.localIP().toString(), "D:Menu  *:Start");
  } else {
    String status = serverReachable ? "Server: OK" : "Server: OFFLINE";
    lcdPrint(status, "*:Start  D:IP");
  }
}

void showColorMenu() {
  currentState = STATE_SELECT_COLOR;
  lcdPrint("Couleur:", "1V 2R 3B 4J 5N");
  Serial.println("\n[COULEUR] 1=Vert 2=Rouge 3=Bleu 4=Jaune 5=Noir | #=Retour");
}

void showGIDInput() {
  currentState = STATE_ENTER_GID;
  currentInput = "";
  lcdPrint("G_ID (4 chif):", "____");
  Serial.println("\n[G_ID] Saisissez 4 chiffres | #=Effacer | retour si vide");
}

void showNameInput() {
  currentState = STATE_ENTER_NAME;
  currentInput = "";
  lcdPrint("Nom (A-D,0-9):", "*=OK  #=Efface");
  Serial.println("\n[NOM] Chars: A-D, 0-9, max 8 | *=Valider | #=Effacer");
}

void showConfirmation() {
  currentState = STATE_CONFIRM;
  String line1 = selectedColor.substring(0, 4) + " " + selectedGID;
  String line2 = selectedName + " #:OK *:NO";
  lcdPrint(line1, line2);
  Serial.printf("\n[CONFIRMATION] Couleur=%s | G_ID=%s | Nom=%s\n",
                selectedColor.c_str(), selectedGID.c_str(), selectedName.c_str());
  Serial.println("  # = Confirmer | * = Annuler");
}

// ============================================================
//  GESTION TOUCHES
// ============================================================
void handleKeyPress(char key) {
  switch (currentState) {

    // --- IDLE ---
    case STATE_IDLE:
      if (key == '*') showColorMenu();
      else if (key == 'D') showIPInfo();
      break;

    // --- SÉLECTION COULEUR ---
    case STATE_SELECT_COLOR:
      if (key >= '1' && key <= '5') {
        int idx = key - '0';
        selectedColor = String(COLOR_FR[idx]);
        Serial.println("✅ Couleur: " + selectedColor);
        lcdPrint("Couleur:", selectedColor);
        delay(1000);
        showGIDInput();
      } else if (key == '#') {
        showMainMenu();
      }
      break;

    // --- SAISIE G_ID ---
    case STATE_ENTER_GID:
      if (key >= '0' && key <= '9') {
        if (currentInput.length() < 4) {
          currentInput += key;

          // Mise à jour LCD ligne 2
          String display = currentInput;
          while (display.length() < 4) display += "_";
          lcd.setCursor(0, 1);
          lcd.print(display);

          Serial.printf("G_ID partiel: %s (%d/4)\n", currentInput.c_str(), currentInput.length());

          // Auto-validation à 4 chiffres
          if (currentInput.length() == 4) {
            if (currentInput == "0000") {
              lcdPrint("SUPPRESSION", selectedColor);
              Serial.println("🗑️ Suppression demandée pour: " + selectedColor);
              selectedGID = "0000";
              selectedName = "Supprime";
              delay(1500);
              showConfirmation();
            } else {
              lcdPrint("Verification...", "G_ID: " + currentInput);
              Serial.println("🔍 Vérification G_ID sur serveur...");

              if (checkGIDOnServer(currentInput)) {
                selectedGID = currentInput;
                Serial.println("✅ G_ID OK: " + selectedGID);
                lcdPrint("G_ID valide!", selectedGID);
                delay(1200);
                showNameInput();
              } else {
                Serial.println("❌ G_ID déjà utilisé: " + currentInput);
                lcdPrint("G_ID deja pris!", "Essayez autre");
                delay(2500);
                showGIDInput();
              }
            }
          }
        }
      } else if (key == '#') {
        if (currentInput.length() > 0) {
          currentInput.remove(currentInput.length() - 1);
          String display = currentInput;
          while (display.length() < 4) display += "_";
          lcd.setCursor(0, 1);
          lcd.print(display);
          Serial.println("🔙 Effacement: " + display);
        } else {
          showColorMenu();
        }
      }
      break;

    // --- SAISIE NOM ---
    case STATE_ENTER_NAME:
      if ((key >= 'A' && key <= 'D') || (key >= '0' && key <= '9')) {
        if (currentInput.length() < 8) {
          currentInput += key;

          // Mise à jour LCD
          lcdClearLine(1);
          lcd.print(currentInput);

          Serial.printf("Nom partiel: %s (%d/8)\n", currentInput.c_str(), currentInput.length());

          // Auto-valider si 8 chars
          if (currentInput.length() == 8) {
            selectedName = currentInput;
            lcdPrint("Nom complet:", selectedName);
            delay(1200);
            showConfirmation();
          }
        }
      } else if (key == '*') {
        if (currentInput.length() > 0) {
          selectedName = currentInput;
          Serial.println("✅ Nom validé: " + selectedName);
          lcdPrint("Nom:", selectedName);
          delay(1000);
          showConfirmation();
        } else {
          lcdPrint("Nom vide!", "Saisissez d'abord");
          delay(1500);
          showNameInput();
        }
      } else if (key == '#') {
        if (currentInput.length() > 0) {
          currentInput.remove(currentInput.length() - 1);
          lcdClearLine(1);
          lcd.print(currentInput);
          Serial.println("🔙 Effacement nom: " + currentInput);
        } else {
          showGIDInput();
        }
      }
      break;

    // --- CONFIRMATION ---
    case STATE_CONFIRM:
      if (key == '#') {
        // CORRIGÉ: Envoi APRÈS avoir mis à jour l'état et affichage
        currentState = STATE_SENDING;
        lcdPrint("Envoi en cours..", "Veuillez patienter");
        Serial.println("\n📤 Envoi vers Rust...");

        // Conversion couleur FR → EN
        int colorIdx = getColorIndex(selectedColor);
        String colorEN = (colorIdx > 0) ? String(COLOR_EN[colorIdx]) : "vert";

        bool ok = sendDataToRust(colorEN, selectedGID, selectedName);

        if (ok) {
          lcdBlink(3);
          lcdPrint("ENVOYE!", "Succes!");
        } else {
          lcdPrint("ERREUR ENVOI", "Voir Serial");
        }
        delay(3000);
        showMainMenu();

      } else if (key == '*') {
        Serial.println("❌ Annulé par l'utilisateur");
        showMainMenu();
      }
      break;

    // --- ENVOI EN COURS ---
    case STATE_SENDING:
      // Ignorer toutes les touches pendant l'envoi
      Serial.printf("⏳ Touche '%c' ignorée (envoi en cours)\n", key);
      break;

    default:
      break;
  }
}

// ============================================================
//  AFFICHAGE INFO RÉSEAU (CORRIGÉ - sans delay bloquant)
// ============================================================
void showIPInfo() {
  Serial.println("\n[RESEAU]");
  Serial.printf("  ESP32 IP:  %s\n", WiFi.localIP().toString().c_str());
  Serial.printf("  Rust API:  http://%s:%d\n", SERVER_IP, SERVER_PORT);
  Serial.printf("  WiFi RSSI: %d dBm\n", WiFi.RSSI());
  Serial.printf("  Serveur:   %s\n", serverReachable ? "ONLINE" : "OFFLINE");

  // Affichage séquentiel rapide sur LCD, sans delay bloquant (on appelle handleClient entre)
  auto showAndHandle = [&](String l1, String l2, int ms) {
    lcdPrint(l1, l2);
    unsigned long t = millis();
    while (millis() - t < ms) {
      server.handleClient();
      delay(10);
    }
  };

  showAndHandle("IP ESP32:", WiFi.localIP().toString(), 2500);
  showAndHandle("Rust: " + String(serverReachable ? "ONLINE" : "OFFLINE"),
                String(SERVER_IP) + ":" + String(SERVER_PORT), 2500);
  showAndHandle("RSSI: " + String(WiFi.RSSI()) + " dBm",
                "CH: " + String(WiFi.channel()), 2000);

  showMainMenu();
}

// ============================================================
//  ANIMATION LCD
// ============================================================
void lcdBlink(int times) {
  for (int i = 0; i < times; i++) {
    lcd.backlight();  delay(200);
    lcd.noBacklight();delay(150);
    lcd.backlight();
  }
}

// ============================================================
//  UTILITAIRES
// ============================================================
const char* getStateStr() {
  switch (currentState) {
    case STATE_IDLE:         return "IDLE";
    case STATE_SELECT_COLOR: return "SELECT_COLOR";
    case STATE_ENTER_GID:    return "ENTER_GID";
    case STATE_ENTER_NAME:   return "ENTER_NAME";
    case STATE_CONFIRM:      return "CONFIRM";
    case STATE_SENDING:      return "SENDING";
    default:                 return "UNKNOWN";
  }
}

int getColorIndex(String colorFR) {
  for (int i = 1; i <= 5; i++) {
    if (colorFR.equals(COLOR_FR[i])) return i;
  }
  return 0;
}

// ============================================================
//  SERVEUR WEB EMBARQUÉ ESP32
// ============================================================
void addCORSHeaders() {
  server.sendHeader("Access-Control-Allow-Origin", "*");
  server.sendHeader("Access-Control-Allow-Methods", "GET, POST, OPTIONS");
  server.sendHeader("Access-Control-Allow-Headers", "Content-Type");
}

void setupWebServer() {
  server.on("/",                            HTTP_GET,  handleRoot);
  server.on("/api/esp32/ping",              HTTP_GET,  handlePing);
  server.on("/api/esp32/status",            HTTP_GET,  handleStatus);
  server.on("/api/esp32/current-data",      HTTP_GET,  handleCurrentData);
  server.on("/api/esp32/command",           HTTP_POST, handleCommand);
  server.on("/api/esp32/validate-web-data", HTTP_POST, handleValidateWebData);
  server.on("/api/color-mappings",          HTTP_GET,  handleColorMappings);

  // Gestion CORS (preflight OPTIONS)
  server.onNotFound([]() {
    addCORSHeaders();
    if (server.method() == HTTP_OPTIONS) {
      server.send(200, "text/plain", "OK");
    } else {
      server.send(404, "application/json", "{\"error\":\"Route non trouvée\"}");
    }
  });

  server.begin();
  Serial.println("✅ Serveur Web ESP32 actif sur port 80");
}

// GET /
void handleRoot() {
  addCORSHeaders();
  String html = "<!DOCTYPE html><html><head><meta charset='utf-8'><title>ESP32</title></head><body>";
  html += "<h2>ESP32 Detection System</h2>";
  html += "<p>IP: " + WiFi.localIP().toString() + "</p>";
  html += "<p>Etat: " + String(getStateStr()) + "</p>";
  html += "<p>Rust: " + String(serverReachable ? "ONLINE" : "OFFLINE") + "</p>";
  html += "<ul>";
  html += "<li>GET /api/esp32/ping</li>";
  html += "<li>GET /api/esp32/status</li>";
  html += "<li>GET /api/esp32/current-data</li>";
  html += "<li>POST /api/esp32/command</li>";
  html += "<li>POST /api/esp32/validate-web-data</li>";
  html += "<li>GET /api/color-mappings</li>";
  html += "</ul></body></html>";
  server.send(200, "text/html", html);
}

// GET /api/esp32/ping
void handlePing() {
  addCORSHeaders();
  StaticJsonDocument<256> doc;
  doc["status"]           = "pong";
  doc["message"]          = "ESP32 operationnel";
  doc["ip"]               = WiFi.localIP().toString();
  doc["wifi_connected"]   = (WiFi.status() == WL_CONNECTED);
  doc["server_connected"] = serverReachable;
  doc["state"]            = getStateStr();
  doc["uptime_ms"]        = millis();
  String out; serializeJson(doc, out);
  server.send(200, "application/json", out);
}

// GET /api/esp32/status
void handleStatus() {
  addCORSHeaders();
  StaticJsonDocument<512> doc;
  doc["status"]             = "online";
  doc["esp32_ip"]           = WiFi.localIP().toString();
  doc["wifi_ssid"]          = WIFI_SSID;
  doc["wifi_rssi"]          = WiFi.RSSI();
  doc["server_rust_ip"]     = SERVER_IP;
  doc["server_rust_port"]   = SERVER_PORT;
  doc["server_connected"]   = serverReachable;
  doc["current_state"]      = getStateStr();
  doc["uptime_ms"]          = millis();
  doc["free_heap"]          = ESP.getFreeHeap();
  String out; serializeJson(doc, out);
  server.send(200, "application/json", out);
}

// GET /api/esp32/current-data
void handleCurrentData() {
  addCORSHeaders();
  StaticJsonDocument<512> doc;
  doc["state"]     = getStateStr();
  doc["timestamp"] = millis();
  doc["esp32_ip"]  = WiFi.localIP().toString();

  int colorIdx = getColorIndex(selectedColor);
  String colorEN = (colorIdx > 0) ? String(COLOR_EN[colorIdx]) : "";

  switch (currentState) {
    case STATE_IDLE:
      doc["has_data"]  = false;
      doc["color"]     = "";
      doc["g_id"]      = "";
      doc["type_name"] = "";
      doc["message"]   = "En attente - Appuyez sur * pour commencer";
      break;
    case STATE_SELECT_COLOR:
      doc["has_data"]  = false;
      doc["color"]     = "";
      doc["g_id"]      = "";
      doc["type_name"] = "";
      doc["message"]   = "Selection de la couleur en cours";
      break;
    case STATE_ENTER_GID:
      doc["has_data"]  = false;
      doc["color"]     = colorEN;
      doc["g_id"]      = currentInput;
      doc["type_name"] = "";
      doc["message"]   = "Saisie G_ID (" + String(currentInput.length()) + "/4)";
      break;
    case STATE_ENTER_NAME:
      doc["has_data"]  = false;
      doc["color"]     = colorEN;
      doc["g_id"]      = selectedGID;
      doc["type_name"] = currentInput;
      doc["message"]   = "Saisie nom (" + String(currentInput.length()) + "/8)";
      break;
    case STATE_CONFIRM:
      doc["has_data"]  = true;
      doc["color"]     = colorEN;
      doc["g_id"]      = selectedGID;
      doc["type_name"] = selectedName;
      doc["message"]   = "En attente confirmation (#=OK, *=Annuler)";
      break;
    case STATE_SENDING:
      doc["has_data"]  = true;
      doc["color"]     = colorEN;
      doc["g_id"]      = selectedGID;
      doc["type_name"] = selectedName;
      doc["message"]   = "Envoi en cours...";
      break;
    default:
      doc["has_data"]  = false;
      doc["message"]   = "Etat inconnu";
      break;
  }

  String out; serializeJson(doc, out);
  server.send(200, "application/json", out);
}

// POST /api/esp32/command
// Body JSON: {"command": "reset"} ou {"command": "get_data"}
void handleCommand() {
  addCORSHeaders();

  if (!server.hasArg("plain")) {
    server.send(400, "application/json", "{\"error\":\"JSON manquant\"}");
    return;
  }

  StaticJsonDocument<256> doc;
  if (deserializeJson(doc, server.arg("plain"))) {
    server.send(400, "application/json", "{\"error\":\"JSON invalide\"}");
    return;
  }

  String cmd = doc["command"] | "";

  if (cmd == "reset") {
    // CORRIGÉ: répondre AVANT de changer l'état
    server.send(200, "application/json", "{\"success\":true,\"message\":\"ESP32 reinitialise\"}");
    showMainMenu();
  } else if (cmd == "get_data") {
    handleCurrentData();
  } else {
    server.send(400, "application/json", "{\"error\":\"Commande inconnue: " + cmd + "\"}");
  }
}

// POST /api/esp32/validate-web-data
// Body JSON: {"color":"vert","g_id":"1234","type_name":"AB12"}
void handleValidateWebData() {
  addCORSHeaders();

  if (!server.hasArg("plain")) {
    server.send(400, "application/json", "{\"error\":\"JSON manquant\"}");
    return;
  }

  StaticJsonDocument<512> doc;
  if (deserializeJson(doc, server.arg("plain"))) {
    server.send(400, "application/json", "{\"error\":\"JSON invalide\"}");
    return;
  }

  String color    = doc["color"]     | "";
  String gid      = doc["g_id"]      | "";
  String typeName = doc["type_name"] | "";

  // Validation de base
  if (color.isEmpty() || gid.isEmpty() || typeName.isEmpty()) {
    server.send(400, "application/json", "{\"error\":\"Champs manquants: color, g_id, type_name\"}");
    return;
  }
  if (gid.length() != 4) {
    server.send(400, "application/json", "{\"error\":\"G_ID doit contenir exactement 4 chiffres\"}");
    return;
  }

  // Répondre immédiatement (CORRIGÉ: pas d'appel bloquant ici)
  StaticJsonDocument<512> resp;
  resp["success"]          = true;
  resp["message"]          = "Donnees recues et affichees sur ESP32";
  resp["data"]["color"]    = color;
  resp["data"]["g_id"]     = gid;
  resp["data"]["type_name"] = typeName;

  String out; serializeJson(resp, out);
  server.send(200, "application/json", out);

  // Afficher sur LCD APRÈS avoir répondu
  lcdPrint("WEB: " + color, gid + " " + typeName);
  Serial.printf("📥 Données web reçues: %s / %s / %s\n", color.c_str(), gid.c_str(), typeName.c_str());
}

// GET /api/color-mappings
void handleColorMappings() {
  addCORSHeaders();
  StaticJsonDocument<512> doc;
  JsonArray arr = doc.createNestedArray("mappings");
  for (int i = 1; i <= 5; i++) {
    JsonObject o = arr.createNestedObject();
    o["keypad_key"] = String(i);
    o["color_fr"]   = COLOR_FR[i];
    o["color_en"]   = COLOR_EN[i];
  }
  doc["total"]     = 5;
  doc["esp32_ip"]  = WiFi.localIP().toString();
  String out; serializeJson(doc, out);
  server.send(200, "application/json", out);
}
