pub enum Item {
    Action { id: &'static str, label: String, enabled: bool },
    Check { id: String, label: String, checked: bool },
    Submenu { label: String, items: Vec<Item> },
    Separator,
}

pub struct MenuState {
    pub connected: bool,
    /// (banderole formatée, a un lien visio) — <= 5
    pub upcoming: Vec<(String, bool)>,
    pub paused: bool,
    pub suppress_during_meeting: bool,
    pub lead_minutes: u32,
}

pub const LEAD_CHOICES: [u32; 5] = [2, 5, 10, 15, 30];

/// Ids des lignes de réunion cliquables (ouvrent le lien visio) — l'index
/// correspond à la position dans `prochains(events, now, 5)`.
pub const MEET_IDS: [&str; 5] = ["meet_0", "meet_1", "meet_2", "meet_3", "meet_4"];

pub fn menu_items(s: &MenuState) -> Vec<Item> {
    let mut items = Vec::new();
    items.push(if s.connected {
        Item::Action { id: "disconnect", label: "Se déconnecter".into(), enabled: true }
    } else {
        Item::Action { id: "connect", label: "Se connecter à Google".into(), enabled: true }
    });
    items.push(Item::Separator);
    items.push(Item::Action { id: "header", label: "Prochaines réunions :".into(), enabled: false });
    if s.upcoming.is_empty() {
        items.push(Item::Action { id: "none", label: "Aucune réunion à venir".into(), enabled: false });
    } else {
        // Ligne activée ⇔ lien visio présent : cliquer ouvre la visio.
        for (i, (line, has_link)) in s.upcoming.iter().take(5).enumerate() {
            items.push(Item::Action { id: MEET_IDS[i], label: line.clone(), enabled: *has_link });
        }
    }
    items.push(Item::Separator);
    items.push(Item::Check { id: "pause".into(), label: "En pause".into(), checked: s.paused });
    items.push(Item::Check {
        id: "suppress_meeting".into(),
        label: "Pas d'avion pendant une réunion".into(),
        checked: s.suppress_during_meeting,
    });
    items.push(Item::Submenu {
        label: "Délai avant réunion".into(),
        items: LEAD_CHOICES
            .iter()
            .map(|&m| Item::Check {
                id: format!("lead_{m}"),
                label: format!("{m} min"),
                checked: s.lead_minutes == m,
            })
            .collect(),
    });
    items.push(Item::Action { id: "fly", label: "Faire passer l'avion".into(), enabled: true });
    items.push(Item::Separator);
    items.push(Item::Action { id: "check_updates", label: "Rechercher des mises à jour".into(), enabled: true });
    items.push(Item::Action { id: "quit", label: "Quitter".into(), enabled: true });
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> MenuState {
        MenuState {
            connected: true,
            upcoming: vec![("09 h 05 — Point produit".into(), false)],
            paused: false,
            suppress_during_meeting: true,
            lead_minutes: 10,
        }
    }

    #[test]
    fn ordre_du_menu_connecte() {
        let items = menu_items(&state());
        // Se déconnecter → sep → en-tête + 1 ligne → sep → pause → anti-réunion
        // → sous-menu délai → fly → sep → update → quit
        assert!(matches!(&items[0], Item::Action { id: "disconnect", label, .. } if label == "Se déconnecter"));
        assert!(matches!(items[1], Item::Separator));
        assert!(matches!(&items[2], Item::Action { label, enabled: false, .. } if label == "Prochaines réunions :"));
        assert!(matches!(&items[3], Item::Action { label, enabled: false, .. } if label == "09 h 05 — Point produit"));
        assert!(matches!(items[4], Item::Separator));
        assert!(matches!(&items[5], Item::Check { id, label, checked: false } if id == "pause" && label == "En pause"));
        assert!(matches!(&items[6], Item::Check { id, label, checked: true } if id == "suppress_meeting" && label == "Pas d'avion pendant une réunion"));
        assert!(matches!(&items[7], Item::Submenu { label, .. } if label == "Délai avant réunion"));
        assert!(matches!(&items[8], Item::Action { id: "fly", label, .. } if label == "Faire passer l'avion"));
        assert!(matches!(items[9], Item::Separator));
        assert!(matches!(&items[10], Item::Action { id: "check_updates", label, .. } if label == "Rechercher des mises à jour"));
        assert!(matches!(&items[11], Item::Action { id: "quit", label, .. } if label == "Quitter"));
        assert_eq!(items.len(), 12);
    }

    #[test]
    fn deconnecte_affiche_se_connecter() {
        let mut s = state();
        s.connected = false;
        let items = menu_items(&s);
        assert!(matches!(&items[0], Item::Action { id: "connect", label, .. } if label == "Se connecter à Google"));
    }

    #[test]
    fn aucune_reunion_affiche_le_placeholder() {
        let mut s = state();
        s.upcoming.clear();
        let items = menu_items(&s);
        assert!(matches!(&items[3], Item::Action { label, enabled: false, .. } if label == "Aucune réunion à venir"));
    }

    #[test]
    fn sous_menu_delai_coche_la_valeur_courante() {
        let items = menu_items(&state());
        let Item::Submenu { items: leads, .. } = &items[7] else { panic!() };
        assert_eq!(leads.len(), 5);
        assert!(matches!(&leads[0], Item::Check { id, label, checked: false } if id == "lead_2" && label == "2 min"));
        assert!(matches!(&leads[2], Item::Check { id, checked: true, .. } if id == "lead_10"));
        assert!(matches!(&leads[4], Item::Check { id, label, checked: false } if id == "lead_30" && label == "30 min"));
    }

    #[test]
    fn plus_de_cinq_reunions_tronque_a_cinq() {
        let mut s = state();
        s.upcoming = (0..7).map(|i| (format!("{i:02} h 00 — Réu {i}"), false)).collect();
        let items = menu_items(&s);
        // en-tête à l'index 2, puis exactement 5 lignes, puis séparateur
        assert!(matches!(&items[3], Item::Action { label, enabled: false, .. } if label == "00 h 00 — Réu 0"));
        assert!(matches!(&items[7], Item::Action { label, enabled: false, .. } if label == "04 h 00 — Réu 4"));
        assert!(matches!(items[8], Item::Separator));
        assert_eq!(items.len(), 16); // 12 + 4 lignes de plus qu'avec 1 réunion
    }

    #[test]
    fn ligne_avec_visio_est_cliquable() {
        let mut s = state();
        s.upcoming = vec![
            ("09 h 05 — Visio".into(), true),
            ("11 h 00 — Sans visio".into(), false),
        ];
        let items = menu_items(&s);
        assert!(matches!(&items[3], Item::Action { id: "meet_0", enabled: true, .. }));
        assert!(matches!(&items[4], Item::Action { id: "meet_1", enabled: false, .. }));
    }
}
