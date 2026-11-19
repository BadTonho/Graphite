# Quebra de linha

Se uma aba do editor quebra as linhas longas é decidido para cada aba, a partir de três coisas, nesta ordem:

1. **A sua escolha para aquela aba.** Exibir → Quebra de linha (`Alt+Z`) alterna a quebra só do editor ativo e mantém no topo o texto que estava no topo. Não é salva e não mexe nos outros editores.
2. **A configuração `editor.wordWrap`**, quando você ou o workspace a definem como `on` ou `off`. Ela vale para todos os arquivos. Mudar a configuração substitui as escolhas anteriores por aba.
3. **O padrão do tipo de arquivo:** Markdown e texto puro quebram, código não.

Assim, textos ficam confortáveis de ler sem nenhuma configuração, e o código mantém as linhas longas e a dobra de código.

## Quais arquivos quebram por padrão

- Markdown: `.md`, `.markdown`, `.mdx`.
- Texto puro: arquivos novos ainda não salvos, `.txt` e `.text`, e os arquivos de texto comuns sem extensão (`README`, `LICENSE`, `NOTICE`, `AUTHORS`, `COPYING`, `CHANGELOG`, `CONTRIBUTING`).
- Todo o resto, incluindo qualquer arquivo de linguagem de programação conhecida e arquivos de dados como `.csv`, não quebra.

## Observações

- Com a quebra ligada, a dobra não está disponível naquela aba e a aba não rola para os lados (veja [Rolagem horizontal](horizontal-scrolling.md)); com ela desligada, uma linha longa rola para os lados.
- O padrão é decidido pelo nome e pela linguagem do arquivo, então salvar um arquivo novo como `notas.md` ou `main.rs` muda o padrão dele.
- Configurações por linguagem (um `editor.wordWrap` que vale para uma linguagem só) ainda não são suportadas.
