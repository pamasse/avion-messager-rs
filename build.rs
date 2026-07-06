fn main() {
    // Icône de l'exe (raccourci, barre des tâches, explorateur).
    // wix/avion.ico est généré par `python wix/make_ico.py` (depuis la racine)
    // à partir des mêmes rectangles que sprite::PLANE_RECTS — à régénérer si
    // le pixel art de l'avion change.
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("wix/avion.ico");
        res.compile().expect("compilation des ressources Windows (icône)");
    }
    println!("cargo:rerun-if-changed=wix/avion.ico");
}
