# Terminal: integração com o shell e links

## Prompts, códigos de saída e histórico

O Graphite lê as sequências padrão de integração com o shell presentes na saída do terminal:

- **OSC 133** marca cada prompt e informa o código de saída do comando que acabou de terminar.
- **OSC 7** informa o diretório de trabalho do shell.

Quando um comando termina, uma barra na margem esquerda do terminal mostra o resultado: verde para código de saída 0, vermelho para qualquer outro código e cinza quando o shell não informou o código.

| Ação | Teclas |
|---|---|
| Ir para o prompt anterior | `Ctrl + Cima` (terminal com foco) |
| Ir para o próximo prompt | `Ctrl + Baixo` (terminal com foco) |

Os saltos entre prompts mantêm o histórico de rolagem do terminal. São guardadas até 1000 marcas por terminal.

### Shells que o Graphite prepara

- **PowerShell** (`pwsh` e Windows PowerShell): o Graphite o inicia com `-NoExit -Command <wrapper do prompt>`. O wrapper mantém o texto do seu prompt, acrescenta as sequências e roda primeiro. Um perfil que já passe `-Command`, `-c` ou `-File` é mantido como você configurou.
- **bash** (incluindo Git Bash): o Graphite define `PROMPT_COMMAND`. Um `~/.bashrc` que atribua `PROMPT_COMMAND` substitui isso, e a integração deixa de funcionar nesse shell.

zsh e fish ainda não são preparados. Os prompts funcionam normalmente, mas não aparecem marcas, barras de código de saída nem saltos entre prompts.

O diretório de trabalho vindo do OSC 7 é guardado com a sessão do terminal e usado para resolver links de arquivos relativos. Ainda não é mostrado no painel.

## Links

Segure `Ctrl` e clique em um texto da saída do terminal:

- Uma URL `https://` abre no navegador padrão. Outros esquemas não são abertos.
- Uma referência `caminho:linha` ou `caminho:linha:coluna` abre o arquivo no editor na posição indicada. Caminhos relativos são resolvidos a partir do diretório de trabalho do shell (OSC 7), quando conhecido, senão a partir da raiz do workspace. Letras de unidade do Windows são aceitas.

Os links são reconhecidos na linha sob o ponteiro, então um link quebrado em duas linhas só é reconhecido em parte.

## Cores

As cores do texto usam a paleta do terminal, os índices de 256 cores e as sequências TrueColor de 24 bits (`ESC[38;2;r;g;bm` e `ESC[48;2;r;g;bm`). O terminal é desenhado pelo renderer GPU do Graphite.
