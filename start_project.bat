@echo off
echo ======================================================
echo   DEMARRAGE DU SYSTEME DE DETECTION DE COULEURS
echo ======================================================

:: Verification du venv
if not exist "venv\Scripts\python.exe" (
    echo [ERREUR] Environnement virtuel introuvable dans le dossier 'venv'.
    echo Veuillez creer l'environnement avec : python -m venv venv
    pause
    exit /b
)

:: Demarrage de Flask
echo [INFO] Lancement du serveur Flask...
echo [INFO] L'interface sera disponible sur http://localhost:5000
start "FLASK BACKEND" cmd /k ".\venv\Scripts\python.exe app.py"

:: Demarrage du Backend Rust (si cargo est present)
where cargo >nul 2>nul
if %ERRORLEVEL% equ 0 (
    echo [INFO] Cargo detecte. Lancement du backend Rust...
    cd backend
    start "RUST BACKEND" cmd /k "cargo run"
    cd ..
) else (
    echo [WARN] Cargo non detecte. Le backend Rust ne sera pas lance.
    echo [WARN] Le mode simulation / Flask seul sera utilise.
)

echo.
echo ======================================================
echo   LE SYSTEME EST EN COURS DE DEMARRAGE
echo ======================================================
echo.
pause
