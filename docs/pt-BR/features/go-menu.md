# Menu Ir

O menu Ir permite navegar entre abas e grupos do editor, arquivos, símbolos, linhas e diagnósticos. A busca de arquivos usa o seletor rápido do workspace. Os seletores de símbolos do editor e do workspace usam as declarações reconhecidas pelo parser de sintaxe do Graphite; os resultados do workspace abrangem todas as raízes e usam o texto ainda não salvo dos arquivos abertos.

Ir para Linha/Coluna aceita uma linha iniciada em 1 ou `linha:coluna`. O submenu Alternar Editor percorre as vistas abertas na ordem visual ou na ordem de uso recente, com opções limitadas ao grupo atual. Alternar Grupo foca um dos cinco primeiros grupos, o próximo ou anterior, ou o grupo mais próximo na direção escolhida.

Voltar e Avançar usam o histórico de navegação do editor. Ir para Definição fica disponível quando o arquivo ativo tem um servidor de linguagem configurado. As ações sem implementação no Graphite ou no servidor de linguagem permanecem visíveis, mas desativadas.
