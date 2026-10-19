//! Text the Rust side shows outside the webview (tray menu, native dialog titles, OS
//! notifications of script requests and routine runs), in each UI
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
    /// Title of the file dialog that picks a setup file to import ("Share setup").
    pub open_setup_file: &'static str,
    /// Title of the save dialog that writes an exported setup file.
    pub save_setup_file: &'static str,
    /// Label of the setup file type in those dialogs.
    pub setup_files: &'static str,
    /// Title of the OS notification for a script run request.
    pub script_request_title: &'static str,
    /// Body of that notification, after the script name.
    pub script_request_review: &'static str,
    /// Title of the OS notification for a routine run that waits for an answer.
    pub routine_needs_you: &'static str,
    /// Body of that notification, after the routine name.
    pub routine_answer: &'static str,
    /// Title of the OS notification for a failed routine run.
    pub routine_failed: &'static str,
    /// Title of the OS notification for a routine run that needs attention.
    pub routine_attention: &'static str,
    /// Body of the failed and needs-attention notifications, after the routine name.
    pub routine_see_run: &'static str,
    /// Title of the OS notification for a routine run that finished (its body is the name).
    pub routine_finished: &'static str,
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
    open_setup_file: "Open a setup file",
    save_setup_file: "Save the setup",
    setup_files: "Setup files",
    script_request_title: "An agent wants to run a script",
    script_request_review: "Review it in Harness Desktop",
    routine_needs_you: "Routine needs you",
    routine_answer: "Answer it in Harness Desktop",
    routine_failed: "Routine failed",
    routine_attention: "Routine needs attention",
    routine_see_run: "See the run in Harness Desktop",
    routine_finished: "Routine finished",
    choose_executable: "Choose the `{binary}` executable",
};

const DE: NativeText = NativeText {
    show_window: "Harness Desktop anzeigen",
    hide_window: "Fenster ausblenden",
    quit: "Harness Desktop beenden",
    open_project_folder: "Projektordner öffnen",
    open_setup_file: "Setup-Datei öffnen",
    save_setup_file: "Setup speichern",
    setup_files: "Setup-Dateien",
    script_request_title: "Ein Agent möchte ein Skript ausführen",
    script_request_review: "In Harness Desktop prüfen",
    routine_needs_you: "Routine braucht dich",
    routine_answer: "Antworte in Harness Desktop",
    routine_failed: "Routine fehlgeschlagen",
    routine_attention: "Routine braucht Aufmerksamkeit",
    routine_see_run: "Lauf in Harness Desktop ansehen",
    routine_finished: "Routine abgeschlossen",
    choose_executable: "Programmdatei `{binary}` auswählen",
};

const ES: NativeText = NativeText {
    show_window: "Mostrar Harness Desktop",
    hide_window: "Ocultar ventana",
    quit: "Salir de Harness Desktop",
    open_project_folder: "Abrir una carpeta de proyecto",
    open_setup_file: "Abrir un archivo de configuración",
    save_setup_file: "Guardar la configuración",
    setup_files: "Archivos de configuración",
    script_request_title: "Un agente quiere ejecutar un script",
    script_request_review: "Revísalo en Harness Desktop",
    routine_needs_you: "Una rutina te necesita",
    routine_answer: "Respóndela en Harness Desktop",
    routine_failed: "Una rutina falló",
    routine_attention: "Una rutina requiere atención",
    routine_see_run: "Mira la ejecución en Harness Desktop",
    routine_finished: "Rutina terminada",
    choose_executable: "Elige el ejecutable `{binary}`",
};

const FR: NativeText = NativeText {
    show_window: "Afficher Harness Desktop",
    hide_window: "Masquer la fenêtre",
    quit: "Quitter Harness Desktop",
    open_project_folder: "Ouvrir un dossier de projet",
    open_setup_file: "Ouvrir un fichier de configuration",
    save_setup_file: "Enregistrer la configuration",
    setup_files: "Fichiers de configuration",
    script_request_title: "Un agent veut exécuter un script",
    script_request_review: "À vérifier dans Harness Desktop",
    routine_needs_you: "Une routine a besoin de vous",
    routine_answer: "Répondez dans Harness Desktop",
    routine_failed: "Échec d’une routine",
    routine_attention: "Une routine demande votre attention",
    routine_see_run: "Voir l’exécution dans Harness Desktop",
    routine_finished: "Routine terminée",
    choose_executable: "Choisir l’exécutable `{binary}`",
};

const PL: NativeText = NativeText {
    show_window: "Pokaż Harness Desktop",
    hide_window: "Ukryj okno",
    quit: "Zakończ Harness Desktop",
    open_project_folder: "Otwórz folder projektu",
    open_setup_file: "Otwórz plik konfiguracji",
    save_setup_file: "Zapisz konfigurację",
    setup_files: "Pliki konfiguracji",
    script_request_title: "Agent chce uruchomić skrypt",
    script_request_review: "Sprawdź go w Harness Desktop",
    routine_needs_you: "Rutyna czeka na Ciebie",
    routine_answer: "Odpowiedz w Harness Desktop",
    routine_failed: "Rutyna nie powiodła się",
    routine_attention: "Rutyna wymaga uwagi",
    routine_see_run: "Zobacz przebieg w Harness Desktop",
    routine_finished: "Rutyna zakończona",
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

            for entry in [
                text.show_window,
                text.hide_window,
                text.quit,
                text.open_project_folder,
                text.open_setup_file,
                text.save_setup_file,
                text.setup_files,
                text.script_request_title,
                text.script_request_review,
                text.routine_needs_you,
                text.routine_answer,
                text.routine_failed,
                text.routine_attention,
                text.routine_see_run,
                text.routine_finished,
            ] {
                assert!(!entry.contains(';'), "{lang}: {entry}");
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
            assert!(text.script_request_review.contains("Harness Desktop"), "{lang}");
            assert!(text.routine_answer.contains("Harness Desktop"), "{lang}");
            assert!(text.routine_see_run.contains("Harness Desktop"), "{lang}");
        }
    }
}
