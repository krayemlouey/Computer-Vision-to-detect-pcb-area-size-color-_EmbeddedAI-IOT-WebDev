COMMENT LANCER LE PROJET
========================

J'ai créé un fichier automatique nommé 'start_project.bat' à la racine du projet.

Pour lancer tout le système :
1. Double-cliquez sur 'start_project.bat'.
2. Deux fenêtres noires vont s'ouvrir (une pour Python/Flask, une pour Rust).
3. Ouvrez votre navigateur sur : http://localhost:5000

Si vous voulez lancer les commandes manuellement :

Etape 1 (Python) :
.\venv\Scripts\python.exe app.py

Etape 2 (Rust - dans un autre terminal) :
cd backend
cargo run

Etape 3 :
Ouvrez http://localhost:5000 dans votre navigateur.
