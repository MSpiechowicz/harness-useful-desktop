import type { config as enConfig } from '../en/config';
import type { AreaMessages } from '../types';

export const config: AreaMessages<typeof enConfig> = {
  'config.title': 'Konfiguracja',
  'config.reload': 'Wczytaj konfigurację ponownie',
  'config.resize': 'Zmień rozmiar listy plików',
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
  'config.detail.filter': 'Filtruj klucze i wartości',
  'config.detail.matches.one': '{count} dopasowanie',
  'config.detail.matches.few': '{count} dopasowania',
  'config.detail.matches.many': '{count} dopasowań',
  'config.detail.matches.other': '{count} dopasowania',
  'config.detail.entries.one': '{count} wpis najwyższego poziomu',
  'config.detail.entries.few': '{count} wpisy najwyższego poziomu',
  'config.detail.entries.many': '{count} wpisów najwyższego poziomu',
  'config.detail.entries.other': '{count} wpisu najwyższego poziomu',
  'config.detail.collapseAll': 'Zwiń wszystko',
  'config.detail.expandAll': 'Rozwiń wszystko',
  'config.detail.treeLabel': 'Ustawienia: {title}',
  'config.detail.secrets': 'Sekrety są tu ukryte i nigdy nie opuszczają backendu.',
};
