# Comandos dos arquivos de configurações e de atalhos

O Graphite não tem um editor de configurações; as configurações são arquivos JSON. Estes comandos os abrem, criando um arquivo vazio quando ele ainda não existe.

| Comando | Arquivo |
| --- | --- |
| Abrir configurações do usuário, Abrir configurações do usuário (JSON), Abrir configurações do aplicativo (JSON), Abrir configurações, Abrir configurações de acessibilidade | `settings.json` no perfil. |
| Abrir configurações do espaço de trabalho, (JSON), Abrir configurações da pasta, (JSON) | `.vscode/settings.json` na pasta do espaço de trabalho. O conteúdo existente nunca é sobrescrito. |
| Abrir atalhos de teclado (JSON) | `keybindings.json` no perfil. |
| Abrir atalhos de teclado padrão (JSON) | Uma lista somente leitura dos atalhos embutidos, como entradas `key`, `command` e `when`. Suas entradas em `keybindings.json` valem depois destas. |

O editor de configurações, as configurações remotas e as configurações padrão brutas não estão disponíveis.
