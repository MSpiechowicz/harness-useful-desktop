export const config = {
  'config.title': 'Configuration',
  'config.resize': 'Resize file list',
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
  'config.detail.filter': 'Filter keys and values',
  'config.detail.matches.one': '{count} match',
  'config.detail.matches.other': '{count} matches',
  'config.detail.entries.one': '{count} top-level entry',
  'config.detail.entries.other': '{count} top-level entries',
  'config.detail.collapseAll': 'Collapse all',
  'config.detail.expandAll': 'Expand all',
  'config.detail.treeLabel': '{title} settings',
  'config.detail.secrets': 'Secrets are masked here and never leave the backend.',
} satisfies Record<string, string>;
