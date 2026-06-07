import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  tutorialSidebar: [
    {
      type: 'category',
      label: '入门指南',
      collapsed: false,
      items: [
        'getting-started/introduction',
        'getting-started/installation',
        'getting-started/quick-start',
        'getting-started/configuration',
      ],
    },
    {
      type: 'category',
      label: '架构设计',
      items: [
        'architecture/overview',
        'architecture/four-layer',
        'architecture/state-management',
        'architecture/data-flow',
      ],
    },
    {
      type: 'category',
      label: '核心功能',
      items: [
        'features/import',
        'features/recognition',
        'features/search',
        'features/deduplication',
        'features/export',
        'features/dashboard',
        'features/watcher',
      ],
    },
    {
      type: 'category',
      label: 'Agent 模块',
      collapsed: false,
      items: [
        'agent/overview',
        'agent/tool-calling',
        'agent/tools-reference',
        'agent/streaming',
        'agent/safety',
        'agent/mcp-server',
      ],
    },
    {
      type: 'category',
      label: '模板引擎',
      items: [
        'template-engine/overview',
        'template-engine/pipeline',
        'template-engine/format-preservation',
      ],
    },
    {
      type: 'category',
      label: '配置与设置',
      items: [
        'config/llm-provider',
        'config/embedding',
        'config/badges',
        'config/constants',
      ],
    },
    {
      type: 'category',
      label: '开发者指南',
      items: [
        'developer/build',
        'developer/project-structure',
        'developer/testing',
        'developer/ci-cd',
      ],
    },
    {
      type: 'category',
      label: 'API 参考',
      items: [
        'api/commands',
        'api/data-models',
      ],
    },
  ],
};

export default sidebars;
