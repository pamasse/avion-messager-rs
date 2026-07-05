use tauri_winrt_notification::Toast;

pub fn reconnect_toast() {
    let _ = Toast::new(Toast::POWERSHELL_APP_ID)
        .title("Avion Messager")
        .text1("Connexion Google expirée — reconnecte ton compte depuis l'icône de la barre système.")
        .show();
}
