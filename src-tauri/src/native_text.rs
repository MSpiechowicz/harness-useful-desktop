//! Text the Rust side shows outside the webview (tray menu, native dialog titles), in each UI
//! language of `harness_business::model::LANGUAGES`. An unknown language code falls back to
//! English. The product name stays untranslated.

/// The native strings of one language.
#[derive(Debug, PartialEq, Eq)]
pub struct NativeText {
    /// Tray menu item that shows and focuses the main window.
    pub show_window: &'static str,
    /// Tray menu item that hides the main window.
    pub hide_window: &'static str,
    /// Tray menu item that quits the app.
    pub quit: &'static str,
    /// Title of the folder dialog that opens a project.
    pub open_project_folder: &'static str,
    /// Title of the file dialog that picks a harness executable; `{binary}` is the binary name.
    choose_executable: &'static str,
}

impl NativeText {
    /// Title of the file dialog that picks the `binary_name` executable.
    pub fn choose_executable(&self, binary_name: &str) -> String {
        self.choose_executable.replace("{binary}", binary_name)
    }
}

const EN: NativeText = NativeText {
    show_window: "Show Harness Desktop",
    hide_window: "Hide window",
    quit: "Quit Harness Desktop",
    open_project_folder: "Open a project folder",
    choose_executable: "Choose the `{binary}` executable",
};

const DE: NativeText = NativeText {
    show_window: "Harness Desktop anzeigen",
    hide_window: "Fenster ausblenden",
    quit: "Harness Desktop beenden",
    open_project_folder: "Projektordner öffnen",
    choose_executable: "Programmdatei `{binary}` auswählen",
};

const ES: NativeText = NativeText {
    show_window: "Mostrar Harness Desktop",
    hide_window: "Ocultar ventana",
    quit: "Salir de Harness Desktop",
    open_project_folder: "Abrir una carpeta de proyecto",
    choose_executable: "Elige el ejecutable `{binary}`",
};

const FR: NativeText = NativeText {
    show_window: "Afficher Harness Desktop",
    hide_window: "Masquer la fenêtre",
    quit: "Quitter Harness Desktop",
    open_project_folder: "Ouvrir un dossier de projet",
    choose_executable: "Choisir l’exécutable `{binary}`",
};

const PL: NativeText = NativeText {
    show_window: "Pokaż Harness Desktop",
    hide_window: "Ukryj okno",
    quit: "Zakończ Harness Desktop",
    open_project_folder: "Otwórz folder projektu",
    choose_executable: "Wybierz plik wykonywalny `{binary}`",
};

/// The native strings for the UI language `lang` (English when it is not a shipped language).
pub fn native_text(lang: &str) -> &'static NativeText {
    match lang {
        "de" => &DE,
        "es" => &ES,
        "fr" => &FR,
        "pl" => &PL,
        _ => &EN,
    }
}

#[cfg(test)]
mod tests {
    use harness_business::model::{DEFAULT_LANGUAGE, LANGUAGES};

    use super::*;

    #[test]
    fn every_language_has_every_string() {
        for lang in LANGUAGES {
            let text = native_text(lang);

            for entry in [text.show_window, text.hide_window, text.quit, text.open_project_folder] {
                assert!(!entry.trim().is_empty(), "{lang} has an empty entry");
            }

            let title = text.choose_executable("omp");
            assert!(title.contains("`omp`"), "{lang}: {title}");
            assert!(!title.contains("{binary}"), "{lang}: {title}");
        }
    }

    #[test]
    fn non_english_languages_are_translated() {
        for lang in LANGUAGES.into_iter().filter(|lang| *lang != DEFAULT_LANGUAGE) {
            assert_ne!(native_text(lang), native_text(DEFAULT_LANGUAGE), "{lang}");
        }
    }

    #[test]
    fn english_strings_are_unchanged() {
        let text = native_text("en");

        assert_eq!(text.show_window, "Show Harness Desktop");
        assert_eq!(text.hide_window, "Hide window");
        assert_eq!(text.quit, "Quit Harness Desktop");
        assert_eq!(text.open_project_folder, "Open a project folder");
        assert_eq!(text.choose_executable("omp"), "Choose the `omp` executable");
    }

    #[test]
    fn unknown_language_falls_back_to_english() {
        for lang in ["", "xx", "PL", "en-US"] {
            assert_eq!(native_text(lang), native_text("en"), "{lang:?}");
        }
    }

    #[test]
    fn product_name_stays_untranslated() {
        for lang in LANGUAGES {
            let text = native_text(lang);

            assert!(text.show_window.contains("Harness Desktop"), "{lang}");
            assert!(text.quit.contains("Harness Desktop"), "{lang}");
        }
    }
}
