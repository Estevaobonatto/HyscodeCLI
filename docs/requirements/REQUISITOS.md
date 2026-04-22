# Requisitos do HyscodeCLI

> **Versão:** 1.0.0-alpha  
> **Última atualização:** 2026-04-22

## 1. Requisitos Funcionais (RF)

### RF-001: Interface de Linha de Comando (CLI)

- **RF-001.1:** A CLI deve aceitar comandos no formato `hyscode <comando> [opções]`
- **RF-001.2:** Deve suportar modo interativo (REPL) para conversação contínua
- **RF-001.3:** Deve suportar execução de comandos únicos (modo não-interativo)
- **RF-001.4:** Deve fornecer ajuda contextual (`--help`, `help <comando>`)
- **RF-001.5:** Deve suportar autocompletion para shells: bash, zsh, fish, PowerShell
- **RF-001.6:** Deve exibir feedback visual durante operações longas (spinner, progress bar)
- **RF-001.7:** Deve suportar modo verbose (`-v`, `-vv`, `-vvv`) para debug

### RF-002: Gerenciamento de Provedores

- **RF-002.1:** Deve permitir cadastrar múltiplos provedores simultaneamente
- **RF-002.2:** Deve suportar provedores: Anthropic, OpenAI, GitHub Copilot, Z.ai, OpenRouter
- **RF-002.3:** Deve suportar provedor próprio Hyscode (SaaS)
- **RF-002.4:** Deve permitir configurar API keys por provedor
- **RF-002.5:** Deve permitir definir provedor padrão
- **RF-002.6:** Deve permitir definir modelo padrão por provedor
- **RF-002.7:** Deve validar conectividade e credenciais do provedor
- **RF-002.8:** Deve listar provedores configurados e seus status
- **RF-002.9:** Deve permitir remoção de provedor configurado

### RF-003: Seleção Dinâmica de Modelo

- **RF-003.1:** Deve listar modelos disponíveis por provedor
- **RF-003.2:** Deve permitir seleção de modelo por comando (`--model`)
- **RF-003.3:** Deve suportar alias para modelos (`fast`, `smart`, `code`)
- **RF-003.4:** Deve permitir configuração de modelos favoritos
- **RF-003.5:** Deve sugerir modelo adequado baseado no contexto (tamanho do prompt, tipo de tarefa)

### RF-004: Modos de Operação do Agente

- **RF-004.1:** Modo Chat: conversação livre com o modelo
- **RF-004.2:** Modo Agent: execução autônoma de tarefas com acesso ao filesystem
- **RF-004.3:** Modo Review: análise de código/diffs com sugestões

### RF-005: Contexto e Memória

- **RF-005.1:** Deve permitir enviar arquivos/diretórios como contexto (`--context`, `@file`)
- **RF-005.2:** Deve respeitar .gitignore e .hyscodeignore para exclusão de contexto
- **RF-005.3:** Deve manter histórico de conversas persistente (local)
- **RF-005.4:** Deve permitir resumir contexto quando exceder limites do modelo
- **RF-005.5:** Deve suportar RAG (Retrieval-Augmented Generation) com índice local de código
- **RF-005.6:** Deve permitir injeção de system prompts customizados

### RF-006: Execução de Ferramentas (Tools / Function Calling)

- **RF-006.1:** Deve expor ferramentas ao modelo: leitura de arquivo, escrita de arquivo, execução de comando, busca em código
- **RF-006.2:** Deve solicitar confirmação do usuário para operações destrutivas (escrita, execução)
- **RF-006.3:** Deve permitir modo auto-approve para operações seguras (leitura, busca)
- **RF-006.4:** Deve registrar log de todas as operações executadas
- **RF-006.5:** Deve permitir rollback de alterações quando possível

### RF-007: Streaming e Resposta

- **RF-007.1:** Deve suportar streaming de respostas em tempo real
- **RF-007.2:** Deve renderizar markdown formatado no terminal
- **RF-007.3:** Deve destacar blocos de código com syntax highlighting
- **RF-007.4:** Deve permitir copiar blocos de código para clipboard
- **RF-007.5:** Deve exibir tokens utilizados e estimativa de custo

### RF-008: Configuração e Personalização

- **RF-008.1:** Deve usar arquivo de configuração TOML (`~/.config/hyscode/config.toml`)
- **RF-008.2:** Deve suportar variáveis de ambiente (`HYSCODE_*`)
- **RF-008.3:** Deve permitir themes/skins para interface
- **RF-008.4:** Deve permitir customização de atalhos de teclado (modo TUI)
- **RF-008.5:** Deve permitir configuração de proxy HTTP/SOCKS

### RF-009: Integração com Git

- **RF-009.1:** Deve detectar repositório git automaticamente
- **RF-009.2:** Deve incluir diff do stage como contexto opcional
- **RF-009.3:** Deve gerar mensagens de commit (`hyscode commit`)
- **RF-009.4:** Deve gerar descrições de PR/MR

### RF-010: Segurança

- **RF-010.1:** Deve armazenar API keys em keyring do SO (Windows Credential, macOS Keychain, Linux Secret Service)
- **RF-010.2:** Deve nunca logar ou exibir API keys em plaintext
- **RF-010.3:** Deve permitir execução em sandbox para comandos do agente (opcional)
- **RF-010.4:** Deve validar permissões de arquivo antes de operações
- **RF-010.5:** Deve suportar modo audit-only (só mostra o que faria, não executa)

---

## 2. Requisitos Não-Funcionais (RNF)

### RNF-001: Performance

- **RNF-001.1:** Tempo de inicialização da CLI &lt; 200ms
- **RNF-001.2:** Streaming de resposta deve iniciar &lt; 2s após envio do prompt
- **RNF-001.3:** Consumo de memória &lt; 128MB em operação normal
- **RNF-001.4:** Indexação de código (RAG) deve processar 10k arquivos em &lt; 30s

### RNF-002: Confiabilidade

- **RNF-002.1:** Deve implementar retry exponencial para falhas de rede
- **RNF-002.2:** Deve suportar timeout configurável por provedor
- **RNF-002.3:** Deve manter histórico mesmo em caso de crash
- **RNF-002.4:** Deve validar integridade de configurações

### RNF-003: Portabilidade

- **RNF-003.1:** Deve suportar Windows 10/11, macOS 12+, Linux (glibc e musl)
- **RNF-003.2:** Binário estático para Linux (musl)
- **RNF-003.3:** Instaladores nativos: MSI (Windows), PKG/DMG (macOS), DEB/RPM/AUR (Linux)
- **RNF-003.4:** Deve funcionar em ambientes CI/CD (non-interactive)

### RNF-004: Usabilidade

- **RNF-004.1:** Mensagens de erro claras e acionáveis
- **RNF-004.2:** Documentação inline completa
- **RNF-004.3:** Consistência com convenções Unix (stdin/stdout, exit codes)
- **RNF-004.4:** Suporte a pipes e redirecionamento

### RNF-005: Extensibilidade

- **RNF-005.1:** Arquitetura plugin-based para novos provedores
- **RNF-005.2:** Hooks para customização de comportamento
- **RNF-005.3:** API interna estável para extensões

### RNF-006: Manutenibilidade

- **RNF-006.1:** Cobertura de testes &gt; 80%
- **RNF-006.2:** Documentação de código (rustdoc) 100% das APIs públicas
- **RNF-006.3:** CI/CD automatizado para build, teste e release
- **RNF-006.4:** Linting estrito (clippy deny warnings)

### RNF-007: Segurança

- **RNF-007.1:** Dependências auditadas regularmente (cargo-audit)
- **RNF-007.2:** Sem panics em código de produção (uso de Result/Option)
- **RNF-007.3:** Sanitização de inputs antes de execução de comandos

---

## 3. Requisitos do Provedor Próprio (Hyscode SaaS)

### RFS-001: API Gateway

- **RFS-001.1:** Expor API REST/HTTP2 compatível com OpenAI API spec
- **RFS-001.2:** Autenticação via API Key única por usuário
- **RFS-001.3:** Rate limiting por usuário e por modelo
- **RFS-001.4:** Logging e tracing de todas as requisições

### RFS-002: Roteamento de Modelos

- **RFS-002.1:** Roteamento inteligente baseado em disponibilidade, custo e performance
- **RFS-002.2:** Fallback automático entre provedores em caso de indisponibilidade
- **RFS-002.3:** Seleção de modelo por parâmetro ou alias
- **RFS-002.4:** Cache de respostas para prompts idênticos (onde aplicável)

### RFS-003: Gestão de Usuários e Billing

- **RFS-003.1:** Cadastro e autenticação de usuários
- **RFS-003.2:** Planos de assinatura (Free, Pro, Enterprise)
- **RFS-003.3:** Cobrança por uso (tokens, requisições, ou tempo)
- **RFS-003.4:** Dashboard de uso e custos
- **RFS-003.5:** Alertas de limite de gasto
- **RFS-003.6:** API Keys gerenciáveis (criar, revogar, rotacionar)

### RFS-004: Administração

- **RFS-004.1:** Painel administrativo para gestão de provedores upstream
- **RFS-004.2:** Configuração de modelos disponíveis e preços
- **RFS-004.3:** Monitoramento de saúde dos provedores upstream
- **RFS-004.4:** Análise de uso e métricas de negócio

### RFS-005: Conformidade

- **RFS-005.1:** Não armazenar dados de prompt/resposta permanentemente (opcional por usuário)
- **RFS-005.2:** GDPR / LGPD compliance
- **RFS-005.3:** Auditoria de acessos administrativos

---

## 4. Matriz de Prioridade


| ID      | Requisito                   | Prioridade | Fase |
| ------- | --------------------------- | ---------- | ---- |
| RF-001  | Interface CLI               | Alta       | 2    |
| RF-002  | Gerenciamento de Provedores | Alta       | 2    |
| RF-007  | Streaming e Resposta        | Alta       | 2    |
| RF-008  | Configuração                | Alta       | 2    |
| RF-004  | Modos de Operação           | Alta       | 3    |
| RF-005  | Contexto e Memória          | Média      | 3    |
| RF-006  | Execução de Ferramentas     | Média      | 3    |
| RF-009  | Integração Git              | Baixa      | 4    |
| RF-010  | Segurança                   | Alta       | 2    |
| RFS-001 | API Gateway                 | Alta       | 4    |
| RFS-002 | Roteamento                  | Alta       | 4    |
| RFS-003 | Billing                     | Média      | 5    |
| RFS-004 | Admin                       | Média      | 5    |


---

## 5. Glossário


| Termo                | Definição                                                                                |
| -------------------- | ---------------------------------------------------------------------------------------- |
| **Agente**           | Sistema autônomo que executa tarefas de codificação com acesso a ferramentas             |
| **Provedor**         | Serviço externo que fornece acesso a modelos de linguagem                                |
| **Modelo**           | Instância específica de LLM (ex: gpt-4o, claude-3.5-sonnet)                              |
| **Contexto**         | Informação adicional enviada ao modelo (código, arquivos, histórico)                     |
| **RAG**              | Retrieval-Augmented Generation — técnica de enriquecimento de prompts com dados externos |
| **TUI**              | Terminal User Interface — interface gráfica no terminal                                  |
| **Provider Service** | Serviço SaaS próprio que abstrai múltiplos provedores                                    |


