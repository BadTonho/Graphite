# Temas de cores e Graphite Simple

**Engrenagem de configurações → Temas** (e **Arquivo → Preferências → Temas**) lista **Escuro** e **Claro**, seguidos de todos os temas de cores instalados, com uma marca no tema ativo. Escolher um tema o aplica na hora e grava `workbench.colorTheme` no `settings.json` do perfil; escolher **Escuro** ou **Claro** remove essa configuração. Os temas vêm de configurações importadas, de extensões ativas que contribuem temas, de plugins nativos e dos temas que acompanham o Graphite.

## Graphite Simple

O Graphite Simple é um tema escuro limpo e neutro que acompanha o aplicativo e é instalado no perfil ao iniciar, então aparece no menu sem nenhuma configuração. Ele também é publicado como pacote de tema independente em [assets/themes/graphite-simple/](../../../assets/themes/graphite-simple/) (`package.json` mais um arquivo JSON de tema, sem código), que pode ser instalado como qualquer extensão que contribui temas.

| Elemento | Cor |
|---|---|
| Fundo do editor | `#1e1e1e` |
| Texto do editor | `#d4d4d4` |
| Barra lateral | `#252526` |
| Barra de atividades | `#333333` |
| Barra de status | `#007acc` |
| Palavra-chave / função / tipo | `#569cd6` / `#dcdcaa` / `#4ec9b0` |
| String / número / comentário | `#ce9178` / `#b5cea8` / `#6a9955` |

Para selecioná-lo sem o menu, use `"workbench.colorTheme": "Graphite Simple"`. Veja a [API de extensões](extension-api.md) para saber como os temas contribuídos são aplicados.
