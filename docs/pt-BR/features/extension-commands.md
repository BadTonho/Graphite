# Comandos de extensões

Comandos que trabalham com a barra lateral de extensões. Estão na paleta de comandos em Extensões, exceto os que recebem um argumento, que servem para serem chamados por outro comando ou por uma extensão com `{"text": "..."}`.

| Comando | O que faz |
| --- | --- |
| Ativar todas as extensões | Ativa toda extensão desativada que já era confiável na versão instalada. Extensões que ainda precisam do aviso de confiança continuam desativadas, e a barra lateral avisa. |
| Desativar todas as extensões instaladas | Para e desativa toda extensão ativada. |
| Mostrar extensões populares / recomendadas / publicadas recentemente | Abre a barra lateral de extensões na aba Open VSX. A aba lista os resultados do Open VSX para uma busca vazia, que é a ordem padrão do próprio Open VSX. |
| Procurar extensões de linguagem (e o mesmo para este arquivo / a linguagem ativa) | Abre a aba Open VSX buscando pela linguagem do arquivo ativo. |
| Procurar extensões (`workbench.extensions.search`) | Abre a aba Open VSX e busca pelo texto dado. |
| Instalar extensão (`workbench.extensions.installExtension`) | Recebe um id `publicador.nome`, baixa a versão mais recente do Open VSX e mostra para aprovação antes de instalar, a mesma pré-visualização de escolher na lista. Um id que não tem a forma `publicador.nome` é recusado. |
| Desinstalar extensão (`workbench.extensions.uninstallExtension`) | Recebe o id de uma extensão instalada, a para e a remove. Um id desconhecido é recusado. |

Indisponível: ativar e desativar só no espaço de trabalho, atualizar tudo, verificar atualizações, recomendações e os comandos sobre depurar o host de extensões.
