# KWS

Cansei de abrir split, navegar no diretório, rodar o comando, abrir outro split, navegar
em outro diretório, rodar outro comando, toda vez que ia trabalhar no projeto. É trabalho
repetitivo e idiota: `docker compose up` num split, `pnpm start:dev` em outro, `pnpm dev`
no terceiro, tudo no diretório certo, na ordem certa, no layout certo.

Você descreve isso uma vez num arquivo de configuração. O kws abre tudo. Fim do problema.

## Pré-requisito: DBus sensível a segurança no Konsole

O kws controla o Konsole via DBus, e o Konsole vem com os métodos de DBus que executam
comandos (`sendText`/`runCommand`) **desativados por padrão** por segurança. Sem habilitar
isso, o kws consegue abrir janelas/abas/splits, mas nenhum comando roda.

Habilite em **Konsole > Configurações > Configurar Konsole > Geral > "Enable the security
sensitive parts of the DBus API"**, ou direto pelo terminal:

```sh
kwriteconfig6 --file konsolerc --group KonsoleWindow --key EnableSecuritySensitiveDBusAPI --type bool true
```

Depois, feche todas as janelas do Konsole abertas e rode o kws de novo (a configuração só
é lida quando uma janela nova é criada). Se você esquecer, o kws detecta isso antes de
tentar abrir qualquer coisa e devolve essa mesma instrução no terminal.

## Uso

Os workspaces ficam em `~/.config/kws/*.toml` e são abertos pelo nome:

```sh
kws financeiro           # sobe usando a `base` definida no arquivo, numa janela nova
kws financeiro .         # sobe usando o diretório atual como `base`
kws financeiro ~/x       # sobe usando ~/x como `base`
kws financeiro --attach  # anexa as abas numa janela do Konsole já aberta, em vez de criar uma nova
```

## Exemplo de configuração

`~/.config/kws/financeiro.toml`:

```toml
# Workspace
name   = "financeiro"           #
base   = "~/proj/financeiro"    # raiz; o cwd dos painéis é relativo a ela
driver = "konsole"              # opcional (default: konsole)

on_timeout = "fail"             # readiness estourou -> aborta (ou "continue")

[env]                           # variáveis herdadas por todos os painéis
NODE_ENV = "development"

# Aba: backend
[[tab]]
title = "backend"
cwd   = "server"                # base local da aba (base + "server")

  # Árvore de splits da aba, descrita como nós nomeados e planos.
  # `root` é a raiz; as folhas (db, api, migrate) são aliases de painel.
  #
  #   ┌────────────┬──────────┐
  #   │            │   api    │
  #   │     db     ├──────────┤   ->  root: db | right (colunas 60/40)
  #   │            │ migrate  │      right: api / migrate (linhas)
  #   └────────────┴──────────┘
  [tab.split.root]
  dir   = "columns"             # filhos lado a lado
  ratio = ["60%", "40%"]        # opcional; iguais se omitido
  parts = ["db", "right"]       # nome de nó OU alias de painel

  [tab.split.right]
  dir   = "rows"                # filhos empilhados
  parts = ["api", "migrate"]

  [[tab.pane]]
  area       = "db"             # prende este painel à folha "db"
  run        = "docker compose up"
  ready_when = { port = 5432, timeout = "60s" }   # "pronto" = porta abriu

  [[tab.pane]]
  area       = "api"
  run        = "pnpm start:dev"
  depends_on = ["db"]                              # espera o db ficar pronto
  ready_when = { http = "http://localhost:3000/health" }

  [[tab.pane]]
  area       = "migrate"
  run        = "pnpm db:migrate"
  depends_on = ["db"]
  ready_when = { exit = 0 }                        # one-shot: espera sair com 0
  hold       = true                                # mantém o painel após terminar

# Aba: frontend
[[tab]]
title = "frontend"
cwd   = "client"
# Aba de painel único: sem [tab.split], o painel preenche a aba e dispensa `area`.

  [[tab.pane]]
  run        = "pnpm dev"
  depends_on = ["api"]          # dependências podem atravessar abas
```

### Layout: árvore de splits

O layout de cada aba é uma árvore de splits. Você descreve os nós no TOML (planos), sem
aninhamento, e o kws monta a estrutura. `root` é obrigatório e é a raiz da árvore; os
outros nós são internos e aparecem em `parts`. Tudo que não for nome de nó é folha, e
toda folha precisa de um `[[tab.pane]]` com o `area` correspondente.

- **`dir`**: `"columns"` (lado a lado) ou `"rows"` (empilhados)
- **`ratio`**: `["60%", "40%"]` ou fatores `[0.6, 0.4]`; se omitido, divide igual
- **`parts`**: filhos na ordem: nome de outro nó ou nome de painel (folha)

Aba com um único painel não precisa de `[tab.split]` nem de `area`.

O kws rejeita configurações com ciclo, folha sem painel, painel sem folha ou `root` faltando.

### Condições de `ready_when`

`depends_on` segura um painel até a dependência estar pronta. "Pronta" significa o que
você definir no `ready_when` dela. Todas aceitam `timeout` opcional.

| Condição                               | Pronto quando…               |
| -------------------------------------- | ---------------------------- |
| `{ delay = "5s" }`                     | passou o tempo               |
| `{ port = 5432 }`                      | a porta TCP abriu            |
| `{ http = "http://.../health" }`       | o endpoint respondeu 2xx     |
| `{ file = "/tmp/app.sock" }`           | o arquivo/socket surgiu      |
| `{ exit = 0 }`                         | o processo saiu com código 0 |
| `{ log = "listening on" }`             | o regex casou na saída       |

