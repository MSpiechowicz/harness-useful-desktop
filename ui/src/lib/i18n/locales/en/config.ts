export const config = {
  'config.title': 'Configuration',
  'config.resize': 'Resize file list',
  'config.drawer.label': 'Files',
  'config.list.noMatch': 'No file matches your search.',
  'config.empty': 'No configuration files were found for this harness.',

  'config.layer.user': 'User',
  'config.layer.profile': 'Profile',
  'config.layer.project': 'Project',
  'config.format.text': 'Text',

  'config.list.label': 'Configuration files',
  'config.list.layer': '{layer} layer',
  'config.list.error': 'Error',
  'config.list.notFound': 'Not found',

  'config.detail.reveal': 'Reveal in file manager',
  'config.detail.readError': 'This file could not be read',
  'config.detail.missingTitle': 'No file here yet',
  'config.detail.missingBody': 'The harness has no settings at this path. Settings from this layer will appear once the file exists.',
  'config.detail.treeLabel': '{title} settings',
  'config.detail.secrets': 'Secrets are masked here and never leave the backend.',

  'config.search.label': 'Search keys and values',
  'config.search.placeholder': 'Search settings',
  'config.search.matches.one': '{count} match',
  'config.search.matches.other': '{count} matches',

  'config.mode.label': 'View',
  'config.mode.structured': 'Structured',
  'config.mode.source': 'Source',

  'config.menu.label': 'More actions',
  'config.menu.collapseAll': 'Collapse all',
  'config.menu.expandAll': 'Expand all',
  'config.menu.copyPath': 'Copy path',

  'config.overview.title': 'Overview',
  'config.overview.entries': 'top-level entries',
  'config.overview.values': 'values',
  'config.overview.depth': 'levels deep',
  'config.overview.secrets': 'secrets masked',

  'config.file.title': 'File',
  'config.file.layer': 'Layer',
  'config.file.format': 'Format',
  'config.file.profile': 'Profile',
  'config.file.path': 'Path',

  'config.profiles.title': 'Profiles',
  'config.profiles.hint': 'Start {harness} with --profile <name> to use one.',
  'config.profiles.less': 'Show fewer',
  'config.profiles.all': 'Show all {count}',
} satisfies Record<string, string>;
