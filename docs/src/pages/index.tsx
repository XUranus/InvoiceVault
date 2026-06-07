import React from 'react';
import Layout from '@theme/Layout';
import Link from '@docusaurus/Link';

export default function Home(): React.ReactElement {
  return (
    <Layout title="票匣 Wiki" description="InvoiceVault 智能发票管理桌面应用文档">
      <main style={{ padding: '80px 20px', textAlign: 'center' }}>
        <h1 style={{ fontSize: '3rem', marginBottom: '16px' }}>票匣</h1>
        <p style={{ fontSize: '1.3rem', color: '#666', marginBottom: '40px' }}>
          智能发票管理桌面应用 — 导入、识别、搜索、导出
        </p>
        <div style={{ display: 'flex', gap: '16px', justifyContent: 'center' }}>
          <Link
            className="button button--primary button--lg"
            to="/getting-started/introduction"
          >
            开始阅读
          </Link>
          <Link
            className="button button--secondary button--lg"
            to="/agent/overview"
          >
            了解 Agent
          </Link>
        </div>

        <div style={{ maxWidth: '800px', margin: '60px auto 0', textAlign: 'left' }}>
          <h2>文档导航</h2>
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px' }}>
            {[
              { title: '🚀 入门指南', desc: '安装、配置、第一次使用', to: '/getting-started/introduction' },
              { title: '🏗️ 架构设计', desc: '四层架构、状态管理、数据流', to: '/architecture/overview' },
              { title: '📋 核心功能', desc: '导入、识别、搜索、去重、导出', to: '/features/import' },
              { title: '🤖 Agent 模块', desc: '工具调用、流式输出、安全机制', to: '/agent/overview' },
              { title: '📊 模板引擎', desc: 'Excel 模板导出、格式保持', to: '/template-engine/overview' },
              { title: '⚙️ 配置与设置', desc: 'LLM、Embedding、Badge 配置', to: '/config/llm-provider' },
              { title: '🔧 开发者指南', desc: '构建、测试、CI/CD', to: '/developer/build' },
              { title: '📖 API 参考', desc: '命令列表、数据模型', to: '/api/commands' },
            ].map((item) => (
              <Link
                key={item.to}
                to={item.to}
                style={{
                  display: 'block',
                  padding: '16px',
                  border: '1px solid #e5e7eb',
                  borderRadius: '8px',
                  textDecoration: 'none',
                  color: 'inherit',
                  transition: 'box-shadow 0.2s',
                }}
              >
                <strong>{item.title}</strong>
                <p style={{ margin: '4px 0 0', fontSize: '0.9em', color: '#666' }}>{item.desc}</p>
              </Link>
            ))}
          </div>
        </div>
      </main>
    </Layout>
  );
}
