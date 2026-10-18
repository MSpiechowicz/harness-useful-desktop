import type { config as enConfig } from '../en/config';
import type { AreaMessages } from '../types';

export const config: AreaMessages<typeof enConfig> = {
  'config.title': 'Konfiguracja',
  'config.resize': 'Zmień rozmiar listy plików',
  'config.drawer.label': 'Pliki',
  'config.list.noMatch': 'Żaden plik nie pasuje do wyszukiwania.',
  'config.empty': 'Nie znaleziono plików konfiguracji dla tego harnessa.',

  'config.layer.user': 'Użytkownik',
  'config.layer.profile': 'Profil',
  'config.layer.project': 'Projekt',
  'config.format.text': 'Tekst',

  'config.list.label': 'Pliki konfiguracji',
  'config.list.layer': 'Warstwa „{layer}”',
  'config.list.error': 'Błąd',
  'config.list.notFound': 'Nie znaleziono',

  'config.detail.reveal': 'Pokaż w menedżerze plików',
  'config.detail.readError': 'Nie udało się odczytać tego pliku',
  'config.detail.missingTitle': 'Brak pliku',
  'config.detail.missingBody': 'Harness nie ma ustawień pod tą ścieżką. Ustawienia tej warstwy pojawią się, gdy plik zacznie istnieć.',
  'config.detail.treeLabel': 'Ustawienia: {title}',
  'config.detail.secrets': 'Sekrety są tu ukryte i nigdy nie opuszczają backendu.',

  'config.search.label': 'Szukaj w kluczach i wartościach',
  'config.search.placeholder': 'Szukaj w ustawieniach',
  'config.search.matches.one': '{count} dopasowanie',
  'config.search.matches.few': '{count} dopasowania',
  'config.search.matches.many': '{count} dopasowań',
  'config.search.matches.other': '{count} dopasowania',

  'config.mode.label': 'Widok',
  'config.mode.structured': 'Strukturalny',
  'config.mode.source': 'Źródło',

  'config.menu.label': 'Więcej akcji',
  'config.menu.collapseAll': 'Zwiń wszystko',
  'config.menu.expandAll': 'Rozwiń wszystko',
  'config.menu.copyPath': 'Kopiuj ścieżkę',

  'config.overview.title': 'Przegląd',
  'config.overview.entries': 'wpisów najwyższego poziomu',
  'config.overview.values': 'wartości',
  'config.overview.depth': 'poziomów zagnieżdżenia',
  'config.overview.secrets': 'ukrytych sekretów',

  'config.file.title': 'Plik',
  'config.file.layer': 'Warstwa',
  'config.file.format': 'Format',
  'config.file.profile': 'Profil',
  'config.file.path': 'Ścieżka',

  'config.profiles.title': 'Profile',
  'config.profiles.hint': 'Uruchom {harness} z opcją --profile <name>, aby użyć jednego.',
  'config.profiles.less': 'Pokaż mniej',
  'config.profiles.all': 'Pokaż wszystkie ({count})',
};
