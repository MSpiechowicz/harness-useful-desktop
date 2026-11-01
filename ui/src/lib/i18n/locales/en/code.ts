export const code = {
  'code.onboarding.title': 'Harness Code',
  'code.onboarding.body': 'Installs the Harness Code MCP server, so every agent can look up where code is defined and who uses it. The Harness Code MCP column on the next step shows which harnesses have it. It follows the Code graph switch in Settings.',
  'code.onboarding.registers': 'Registers `harness-code mcp` as an MCP server in each detected harness, while the Code graph is on in Settings.',
  'code.onboarding.mcp.title': 'Harness Code MCP',
  'code.onboarding.mcp.body': 'Lets your agents look up where code is defined and who uses it, by registering Harness Code as an MCP server.',
  'code.onboarding.mcp.note': 'Only the harness-code entry is added, running the installed server. Adding it turns the Code graph on in Settings. A file the app cannot edit safely is left alone.',
  'code.onboarding.mcp.needsCode': 'Needs Harness Code. Include its install above, or install it yourself first.',
  'code.onboarding.effects': 'The code installer also',
  'code.onboarding.done': 'Code',
  'code.onboarding.doneMcp': 'Code MCP',
} satisfies Record<string, string>;
