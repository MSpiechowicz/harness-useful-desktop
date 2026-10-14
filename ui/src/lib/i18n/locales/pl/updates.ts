import type { updates as enUpdates } from '../en/updates';
import type { AreaMessages } from '../types';

export const updates: AreaMessages<typeof enUpdates> = {
  'updates.installerCloses': 'Instalator zamknie Harness Desktop i otworzy go ponownie po zakończeniu.',
  'updates.restartStopsAgents': 'Działający agenci zatrzymują się przy ponownym uruchomieniu aplikacji.',
  'updates.restartWarning.one': '{count} działający czat zostanie zatrzymany, a każda odpowiedź w toku zostanie utracona.',
  'updates.restartWarning.few': '{count} działające czaty zostaną zatrzymane, a każda odpowiedź w toku zostanie utracona.',
  'updates.restartWarning.many': '{count} działających czatów zostanie zatrzymanych, a każda odpowiedź w toku zostanie utracona.',
  'updates.restartWarning.other': '{count} działających czatów zostanie zatrzymanych, a każda odpowiedź w toku zostanie utracona.',

  'updates.unit.kb': 'KB',
  'updates.unit.mb': 'MB',
  'updates.progress.of': '{downloaded} z {total}',

  'updates.failure.network.check': 'Nie można połączyć się z serwerem aktualizacji. Sprawdź połączenie i spróbuj ponownie.',
  'updates.failure.network.install': 'Pobieranie zostało przerwane. Nic nie zmieniono; spróbuj ponownie.',
  'updates.failure.verification': 'Nie udało się zweryfikować aktualizacji i nie została zainstalowana.',
  'updates.failure.manifest': 'Informacje o aktualizacji były niepełne albo nie zawierają niczego dla tego systemu. Spróbuj ponownie później.',
  'updates.failure.installer': 'Instalator nie zakończył pracy albo prośba o uprawnienia została anulowana. Nic nie zmieniono.',
  'updates.failure.busy': 'Aktualizacja jest już instalowana.',
  'updates.failure.stale': 'Ta aktualizacja nie jest już dostępna. Sprawdź ponownie.',
  'updates.failure.unknown': 'Coś poszło nie tak podczas aktualizacji. Nic nie zmieniono.',

  'updates.status.idle': 'W tej sesji jeszcze nie sprawdzano.',
  'updates.status.checking': 'Sprawdzanie aktualizacji…',
  'updates.status.upToDate': 'Masz aktualną wersję. Sprawdzono o {time}.',
  'updates.status.notConfigured': 'Ta kompilacja nie może sprawdzać aktualizacji.',
  'updates.status.available': 'Dostępna jest wersja {version}.',
  'updates.status.downloading': 'Pobieranie wersji {version}…',
  'updates.status.installing': 'Instalowanie wersji {version}…',
  'updates.status.readyToRelaunch': 'Wersja {version} jest zainstalowana. Uruchom ponownie, aby dokończyć.',
  'updates.status.failedCheck': 'Nie udało się sprawdzić aktualizacji.',
  'updates.status.failedInstall': 'Aktualizacja nie została zainstalowana.',


  'updates.install.restart': 'Zainstaluj i uruchom ponownie',

  'updates.notice.label': 'Aktualizacja',
  'updates.notice.available': 'Dostępna jest wersja {version}',
  'updates.notice.updating': 'Aktualizowanie do {version}',
  'updates.notice.installed': 'Wersja {version} jest zainstalowana',
  'updates.notice.failed': 'Aktualizacja nie została zainstalowana',
  'updates.notice.restartBody': 'Uruchom ponownie, aby dokończyć. Działający agenci zatrzymują się przy ponownym uruchomieniu aplikacji.',
  'updates.notice.install': 'Zainstaluj',
  'updates.notice.view': 'Zobacz',
  'updates.notice.later': 'Później',
  'updates.notice.finish': 'Dokończ aktualizację',
  'updates.notice.restart': 'Uruchom ponownie teraz',
  'updates.notice.retry': 'Spróbuj ponownie',
  'updates.notice.dismiss': 'Odrzuć',

  'updates.progress.label': 'Postęp aktualizacji',
  'updates.progress.installing': 'Instalowanie…',
  'updates.progress.downloading': 'Pobieranie…',

  'updates.restart.title': 'Uruchomić ponownie teraz?',
  'updates.restart.confirm': 'Uruchom ponownie teraz',
  'updates.restart.finish': 'Dokończ aktualizację',
  'updates.restart.keepWorking': 'Pracuj dalej',
  'updates.restart.failed': 'Nie udało się uruchomić ponownie',
};
