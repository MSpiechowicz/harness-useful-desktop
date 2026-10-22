import type { config as enConfig } from '../en/config';
import type { AreaMessages } from '../types';

export const config: AreaMessages<typeof enConfig> = {
  'config.title': 'Configuração',
  'config.resize': 'Redimensionar a lista de arquivos',
  'config.drawer.label': 'Arquivos',
  'config.list.noMatch': 'Nenhum arquivo corresponde à sua pesquisa.',
  'config.empty': 'Nenhum arquivo de configuração foi encontrado para este harness.',

  'config.layer.user': 'Usuário',
  'config.layer.profile': 'Perfil',
  'config.layer.project': 'Projeto',
  'config.format.text': 'Texto',

  'config.list.label': 'Arquivos de configuração',
  'config.list.layer': 'Camada {layer}',
  'config.list.error': 'Erro',
  'config.list.notFound': 'Não encontrado',

  'config.detail.reveal': 'Mostrar no gerenciador de arquivos',
  'config.detail.readError': 'Não foi possível ler este arquivo',
  'config.detail.missingTitle': 'Ainda não há arquivo aqui',
  'config.detail.missingBody': 'O harness não tem configurações neste caminho. As configurações desta camada aparecerão quando o arquivo existir.',
  'config.detail.treeLabel': 'Configurações de {title}',
  'config.detail.secrets': 'Os segredos ficam mascarados aqui e nunca saem do backend.',

  'config.search.label': 'Pesquisar chaves e valores',
  'config.search.placeholder': 'Pesquisar configurações',
  'config.search.matches.one': '{count} resultado',
  'config.search.matches.other': '{count} resultados',

  'config.mode.label': 'Visualização',
  'config.mode.structured': 'Estruturada',
  'config.mode.source': 'Código-fonte',

  'config.menu.label': 'Mais ações',
  'config.menu.collapseAll': 'Recolher tudo',
  'config.menu.expandAll': 'Expandir tudo',
  'config.menu.copyPath': 'Copiar caminho',

  'config.overview.title': 'Visão geral',
  'config.overview.entries': 'entradas de nível superior',
  'config.overview.values': 'valores',
  'config.overview.depth': 'níveis de profundidade',
  'config.overview.secrets': 'segredos mascarados',

  'config.file.title': 'Arquivo',
  'config.file.layer': 'Camada',
  'config.file.format': 'Formato',
  'config.file.profile': 'Perfil',
  'config.file.path': 'Caminho',

  'config.profiles.title': 'Perfis',
  'config.profiles.hint': 'Inicie o {harness} com --profile <name> para usar um.',
  'config.profiles.less': 'Mostrar menos',
  'config.profiles.all': 'Mostrar todos os {count}',
};
