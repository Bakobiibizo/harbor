# Run Bob instance with different ports
# Set environment variables for Bob's data directory and ports

$env:HARBOR_DATA_DIR = "D:\apps\chat-app\bob-data"
$env:VITE_DEV_SERVER_PORT = "1421"

# Run tauri dev with custom vite port
npm run tauri dev -- -- -- --port 1421
