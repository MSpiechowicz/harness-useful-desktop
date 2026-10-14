import type { settings as enSettings } from '../en/settings';
import type { AreaMessages } from '../types';

export const settings: AreaMessages<typeof enSettings> = {
  'settings.title': 'Ustawienia',
  'settings.context': 'Zmiany działają od razu i są zapisywane na tym urządzeniu',
  'settings.nav.label': 'Sekcje ustawień',

  'settings.section.appearance': 'Wygląd',
  'settings.section.startup': 'Uruchamianie',
  'settings.section.harnesses': 'Harnessy',
  'settings.section.projects': 'Projekty',
  'settings.section.updates': 'Aktualizacje',
  'settings.section.privacy': 'Prywatność i bezpieczeństwo',
  'settings.section.about': 'O aplikacji',

  'settings.keywords.appearance': 'motyw animacje ruch ciemny jasny animacja język',
  'settings.keywords.startup': 'intro start uruchamianie ekran powitalny',
  'settings.keywords.harnesses': 'plik wykonywalny binarka omp pi ścieżka konfiguracja',
  'settings.keywords.projects': 'ostatnie foldery',
  'settings.keywords.updates': 'wersja aktualizacja nowa wydanie instalacja sprawdź',
  'settings.keywords.privacy': 'dane telemetria diagnostyka wyczyść prywatność',
  'settings.keywords.about': 'wersja kompilacja o aplikacji',

  'settings.theme.title': 'Motyw',
  'settings.theme.description': 'Dopasuj do systemu albo wybierz własny. Zmiana od razu pokazuje podgląd.',
  'settings.theme.system': 'Systemowy',
  'settings.theme.dark': 'Ciemny',
  'settings.theme.light': 'Jasny',

  'settings.language.title': 'Język',
  'settings.language.description': 'Język całej aplikacji. Zmiana działa od razu.',

  'settings.thinking.title': 'Pokazuj tok rozumowania',
  'settings.thinking.description': 'Rozumowanie agenta, zwinięte pod każdą odpowiedzią. Można je też przełączyć w nagłówku czatu.',
  'settings.aiNames.title': 'Nazywaj czaty za pomocą AI',
  'settings.aiNames.description': 'Krótka nazwa i opis każdego czatu, napisane przez agenta czatu zaraz po pierwszej wiadomości.',
  'settings.aiNames.cost': 'Kosztuje jedno bardzo małe zapytanie na czat, na modelu czatu.',
  'settings.aiNames.sent': 'Wysyła tylko pierwsze 500 znaków tej pierwszej wiadomości.',
  'settings.aiNames.off': 'Wyłączone: nazwą są pierwsze wpisane słowa i nic nie jest wysyłane.',

  'settings.motion.title': 'Animacje',
  'settings.motion.description': 'Ograniczone zostawia proste zanikanie; Wyłączone wyłącza wszystkie animacje. Ustawienie ograniczenia ruchu w systemie zawsze ma pierwszeństwo.',
  'settings.motion.full': 'Pełne',
  'settings.motion.reduced': 'Ograniczone',
  'settings.motion.off': 'Wyłączone',

  'settings.intro.title': 'Odtwarzaj intro przy uruchomieniu',
  'settings.intro.description': 'Krótkie animowane powitanie po otwarciu aplikacji. Dowolny klawisz lub kliknięcie je pomija.',
  'settings.sound.title': 'Dźwięk intro',
  'settings.sound.description': 'Dźwięk logo podczas intro. Nie jest odtwarzany, gdy intro jest wyłączone lub pominięte.',
  'settings.replay.title': 'Odtwórz intro ponownie',
  'settings.replay.motionOff': 'Animacje są wyłączone; włącz je, aby odtworzyć intro ponownie.',
  'settings.replay.ready': 'Obejrzyj je jeszcze raz.',

  'settings.harness.binary': 'Plik wykonywalny {name}',
  'settings.harness.notFound': 'Nie znaleziono na tym komputerze',
  'settings.harness.choose': 'Wybierz…',
  'settings.harness.reset': 'Przywróć',
  'settings.setup.title': 'Konfiguracja początkowa',
  'settings.setup.description': 'Wykryj harnessy jeszcze raz, wybierz codzienny harness i projekt.',
  'settings.setup.run': 'Uruchom konfigurację ponownie',

  'settings.recents.title': 'Zapamiętuj ostatnie projekty',
  'settings.recents.description': 'Przechowuj listę otwieranych folderów. Wyłączenie czyści listę.',

  'settings.updates.version': 'Wersja {version}',
  'settings.updates.check': 'Sprawdź teraz',
  'settings.updates.policy.title': 'Przy uruchomieniu aplikacji',
  'settings.updates.policy.label': 'Zasada aktualizacji',
  'settings.updates.policy.startupAsk': 'Najpierw zapytaj',
  'settings.updates.policy.autoInstall': 'Automatycznie',
  'settings.updates.policy.off': 'Wyłączone',
  'settings.updates.policy.startupAsk.hint': 'Szuka nowej wersji przy uruchomieniu aplikacji i pyta przed jej instalacją.',
  'settings.updates.policy.autoInstall.hint': 'Szuka przy uruchomieniu aplikacji i instaluje nową wersję. Sam wybierasz, kiedy uruchomić ponownie.',
  'settings.updates.policy.off.hint': 'Nigdy nie szuka sam. Użyj Sprawdź teraz, gdy chcesz.',
  'settings.updates.companions.title': 'Sprawdzaj aktualizacje towarzyszy',
  'settings.updates.companions.description':
    'Po otwarciu Ecosystem pyta GitHub o najnowsze wydania Harness Dashboard, Skills i Sidebar, najwyżej co sześć godzin, gdy aplikacja działa. GitHub widzi Twój adres IP i wersję aplikacji, ale nie to, których towarzyszy używasz.',
  'settings.updates.notConfigured': 'Ta wersja nie jest przygotowana na aktualizacje, więc nie będzie szukać nowych wersji. Nic się nie dzieje. Nowe wersje pobierzesz ze strony wydań.',
  'settings.updates.openReleases': 'Otwórz wydania',
  'settings.updates.retry': 'Spróbuj ponownie',
  'settings.updates.notes': 'Informacje o wydaniu',
  'settings.updates.install': 'Zainstaluj aktualizację',
  'settings.updates.finish': 'Dokończ aktualizację',
  'settings.updates.restart': 'Uruchom ponownie, aby dokończyć',

  'settings.about.label': 'O aplikacji',
  'settings.about.version': 'Wersja {version} · {os} {arch}',
  'settings.about.versionDebug': 'Wersja {version} · {os} {arch} · kompilacja debugowa',
  'settings.about.localOnly': 'Tylko lokalnie',
  'settings.about.links': 'Linki projektu',
  'settings.about.source': 'Kod źródłowy',
  'settings.about.releases': 'Wydania',
  'settings.about.licence': 'Licencja GPL-3.0-or-later',

  'settings.privacy.label': 'Co Harness Desktop robi z danymi',
  'settings.privacy.stored.title': 'Zapisane na tym urządzeniu',
  'settings.privacy.stored.text':
    'Tylko preferencje aplikacji: motyw, język, animacje, intro, domyślny harness, zasada aktualizacji, postęp konfiguracji, rozmiar i położenie okna, układ diffa, ostatnio otwarty folder, ścieżki ostatnich projektów (jeśli są zapamiętywane) oraz wybrane przez Ciebie ścieżki plików wykonywalnych. Są w folderze konfiguracji aplikacji Twojego systemu. Nie zapisujemy treści czatów, tokenów ani haseł.',
  'settings.privacy.neverRead.title': 'Nigdy nie czytane',
  'settings.privacy.neverRead.text':
    'Magazyny poświadczeń (auth.json, mcp-auth.json, credentials, pliki .env), klucze prywatne (.pem, .key, .p12, klucze SSH), bazy danych (.db, .sqlite), pliki logów, pamięci podręczne blobów agentów i sesje terminala. Sprawdzanie obejmuje też dowiązania symboliczne.',
  'settings.privacy.redacted.title': 'Pokazywane z zamazaniem',
  'settings.privacy.redacted.text':
    'Wartości konfiguracji wyglądające na sekrety są ukryte: klucze API i tokeny, zmienne środowiskowe i nagłówki serwerów MCP, pola uwierzytelniania i OAuth, wartości !command (nigdy nie uruchamiane) oraz dane logowania w adresach URL. Odwołania do środowiska, takie jak $NAME, pokazują tylko nazwę. Komunikaty błędów agentów i wyjście powłoki są zamazywane, zanim trafią na ekran.',
  'settings.privacy.network.title': 'Sieć',
  'settings.privacy.network.text':
    'Sama aplikacja wysyła dwa rodzaje zapytań, oba do GitHuba, i oba można wyłączyć. Sprawdzanie aktualizacji pobiera plik wydania latest.json; dzieje się to tylko wtedy, gdy ta kompilacja ma klucz aktualizacji, i zależy od Twojego ustawienia aktualizacji. Sprawdzanie towarzyszy pyta GitHuba o numer najnowszego wydania Harness Dashboard, Skills i Sidebar po otwarciu Ecosystem, najwyżej co sześć godzin, gdy aplikacja działa (nieudane sprawdzenie jest ponawiane po kilku minutach). Zawsze pyta o wszystkie trzy, więc nie zdradza, których używasz; GitHub widzi tylko Twój adres IP i wersję aplikacji. Linki otwierają się w przeglądarce tylko po kliknięciu i tylko dla stron projektów i harnessów, chyba że poniżej zezwolisz na każdy link https. Widżety Harness Dashboard korzystają z lokalnego procesu przez stdio, bez sieci. CLI harnessów łączą się z ich dostawcami tak jak w Twoim terminalu, a akcje rozszerzeń uruchamiają CLI, które może pobierać pakiety, dopiero po Twoim potwierdzeniu.',
  'settings.privacy.telemetry.title': 'Bez telemetrii',
  'settings.privacy.telemetry.text': 'Bez analityki, bez śledzenia, bez kont, bez logowania.',
  'settings.privacy.agents.title': 'Agenci',
  'settings.privacy.agents.text':
    'Agenci działają na Twoim komputerze, z Twoim środowiskiem i konfiguracją. Harness Desktop nigdy nie zmienia ustawień Twojego harnessa; wybór zatwierdzania dla czatu dotyczy tylko tego czatu, a aktywny tryb widać w nagłówku czatu, jeśli harness go ma. Polecenia powłoki z pola wiadomości zawsze wymagają potwierdzenia. Ta aplikacja niczego nie wysyła.',
  'settings.privacy.git.title': 'Git',
  'settings.privacy.git.text':
    'Widok Zmiany uruchamia Twój lokalny git tylko do odczytu (status, diff, lista gałęzi), z wyłączonymi pagerami, zewnętrznymi programami diff i monitorami systemu plików oraz z wyłączonymi pytaniami o dane logowania.',
  'settings.privacy.logs.title': 'Logi',
  'settings.privacy.logs.text':
    'Lokalny plik logu w folderze logów systemu zapisuje, co robi aplikacja, nigdy to, co mówisz. Nie zawiera promptów, treści czatów ani zawartości plików i jest rotowany przy 1 MiB.',

  'settings.privacy.openLink.title': 'Otwieraj każdy link internetowy',
  'settings.privacy.openLink.description':
    'Linki w czatach, np. do pull requestu otwartego przez agenta, otwierają się w przeglądarce. Wyłączone: otwierają się tylko strony projektów i harnessów. Zawsze tylko https.',
  'settings.privacy.recents.title': 'Ostatnie projekty',
  'settings.privacy.recents.description': 'Zapomnij każdy zapamiętany folder. Jeśli otwarty projekt jest na liście, zostanie też zamknięty.',
  'settings.privacy.recents.clear': 'Wyczyść ostatnie projekty',
  'settings.privacy.recents.confirmTitle': 'Wyczyścić ostatnie projekty?',
  'settings.privacy.recents.confirmDescription': 'Każdy zapamiętany folder zostanie zapomniany. Jeśli otwarty projekt jest na liście, zostanie też zamknięty. Twoje pliki nie są ruszane.',
  'settings.privacy.recents.confirmLabel': 'Wyczyść listę',
  'settings.privacy.recents.cleared': 'Wyczyszczono ostatnie projekty',
  'settings.privacy.diagnostics.title': 'Diagnostyka',
  'settings.privacy.diagnostics.description': 'Kopiuje wersję aplikacji, systemu i harnessów do zgłoszenia błędu. Bez ścieżek, projektów i treści.',
  'settings.privacy.diagnostics.copy': 'Kopiuj diagnostykę',
  'settings.privacy.diagnostics.copied': 'Skopiowano diagnostykę',
  'settings.privacy.diagnostics.copiedDescription': 'Tylko wersja aplikacji, systemu i harnessów. Bez ścieżek i treści.',
  'settings.privacy.diagnostics.failed': 'Nie udało się skopiować',
  'settings.privacy.diagnostics.failedDescription': 'Schowek jest niedostępny.',
};
