import type { harness as enHarness } from '../en/harness';
import type { AreaMessages } from '../types';

export const harness: AreaMessages<typeof enHarness> = {
  'harness.boundary.setup': 'Konfiguracja',
  'harness.boundary.intro': 'Intro',
  'harness.boundary.notifications': 'Powiadomienia',

  'harness.gate.noneTitle': 'Nie znaleziono harnessa',
  'harness.gate.noneBody': 'Harness Desktop działa z OMP, PI i OpenCode. Uruchom konfigurację, aby je wykryć.',
  'harness.gate.missingTitle': '{name} nie jest zainstalowany',
  'harness.gate.setup': 'Skonfiguruj {name}',
  'harness.gate.retry': 'Spróbuj ponownie',
  'harness.gate.banner': '{name} nie jest zainstalowany; widać to, co jest na dysku.',

  'harness.gate.config.missingBody': 'Zainstaluj go albo wskaż Harness Desktop istniejący plik wykonywalny, aby zobaczyć tutaj jego konfigurację.',
  'harness.gate.config.errorTitle': 'Nie udało się odczytać konfiguracji',
  'harness.gate.config.loading': 'Ładowanie konfiguracji',
  'harness.gate.config.refreshing': 'Odświeżanie konfiguracji',
  'harness.gate.extensions.missingBody': 'Zainstaluj go albo wskaż Harness Desktop istniejący plik wykonywalny, aby zobaczyć tutaj jego rozszerzenia.',
  'harness.gate.extensions.errorTitle': 'Nie udało się odczytać rozszerzeń',
  'harness.gate.extensions.loading': 'Ładowanie rozszerzeń',
  'harness.gate.extensions.refreshing': 'Odświeżanie rozszerzeń',

  'harness.switcher.label': 'Przełącz harness',

  'harness.toast.scan': 'Nie udało się wyszukać harnessów',
  'harness.toast.pickBinary': 'Nie można użyć tego pliku',
  'harness.toast.clearBinary': 'Nie udało się zresetować pliku wykonywalnego',
  'harness.toast.recents': 'Nie udało się odczytać ostatnich projektów',
  'harness.toast.pickFolder': 'Nie można otworzyć tego folderu',
  'harness.toast.openProject': 'Nie można otworzyć projektu',
  'harness.toast.closeProject': 'Nie można zamknąć projektu',
  'harness.toast.forgetProject': 'Nie udało się zapomnieć projektu',
};
