# Janela interativa e REPL

Uma janela interativa é um notebook ligado a um kernel com uma caixa de entrada sob as células, para experimentar código e guardar o que aconteceu. Tudo o que é executado vira uma célula de código com suas saídas, então a janela é um notebook: usa o mesmo kernel, as mesmas saídas e os mesmos comandos de célula do [editor de notebook](notebook-editor.md).

| Comando | O que faz |
| --- | --- |
| Abrir janela interativa | Abre uma nova janela em uma aba chamada Interativo. |
| Abrir REPL | Abre o mesmo, com as teclas de um REPL: Enter executa. |
| Executar a seleção ou a linha na janela interativa | A partir de um editor de texto: envia o texto selecionado, ou a linha atual quando nada está selecionado, à janela interativa e o executa. A janela é aberta na primeira vez e reaproveitada depois. A extensão do arquivo (`.py`, `.jl`, `.r`, `.js`) diz em que linguagem o código está, o que escolhe o kernel sem perguntar quando só um kernel roda aquela linguagem. |

## A caixa de entrada

A caixa fica acoplada embaixo, cresce com o texto até oito linhas, e fica com o teclado quando a janela abre e depois de cada execução.

| Tecla | Janela | REPL |
| --- | --- | --- |
| Enter | Nova linha | Executa a entrada |
| Shift+Enter | Executa a entrada | Nova linha |
| Ctrl+Enter | Executa a entrada | Executa a entrada |
| Cima / Baixo na primeira / última linha | Entrada anterior / seguinte do histórico | Igual |
| Escape | Entrega o teclado às células acima | Igual |

Digitar, Tab, Backspace, Delete, as setas, Home e End (Shift seleciona), Ctrl+`A`, Ctrl+`C`, Ctrl+`X`, Ctrl+`V`, Ctrl+`Z` e Ctrl+`Y` funcionam como no editor de texto, dentro da caixa. Um clique na caixa coloca o cursor ali.

Executar inicia o kernel na primeira vez (veja **Escolhendo o kernel** no editor de notebook). Se nenhum kernel puder ser iniciado, a entrada continua na caixa e a barra de status diz o motivo. Uma entrada em branco não executa nada. Percorrer o histórico guarda o que você estava digitando, e voltar além da entrada mais nova o restaura. Executar a mesma entrada duas vezes seguidas a coloca no histórico uma vez só.

## O histórico e as células

As células acima da caixa são o histórico. Com o teclado nelas (Escape, ou **Focar o histórico interativo**) todos os comandos de notebook valem: copiar, excluir, mover, mudar o tipo, recolher, executar de novo. **Focar o último item executado** vai à célula mais nova. **Focar a entrada interativa** devolve o teclado à caixa, **Limpar a entrada interativa** a esvazia, e **Rolar para o topo** e **Rolar para o fim** movem a janela.

## Salvando

A janela não pergunta nada ao ser fechada: o histórico dela é rascunho. Salvá-la (Salvar, ou Salvar como) grava o histórico como um arquivo `.ipynb`, e daí em diante ela é um notebook como outro qualquer, com a marca de modificado de sempre. A próxima abertura desse arquivo é um notebook comum, sem a caixa de entrada.
