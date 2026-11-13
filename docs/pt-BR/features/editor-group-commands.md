# Comandos de grupos e abas de editor

Comandos que movem editores entre grupos e ao longo da barra de abas, criam e juntam grupos e fecham editores em lote. Estão na paleta de comandos, em Workbench.

## Mover e copiar editores

| Comando | O que faz |
| --- | --- |
| Mover editor para o próximo / anterior / primeiro / último grupo | Move a aba ativa para esse grupo, na ordem visual, e foca o grupo. Um grupo que perde a última aba fecha. Se o grupo de destino já mostra o mesmo documento, essa aba é ativada e a aba movida é descartada. Recusado quando só há um grupo. |
| Dividir editor para o próximo / anterior / primeiro / último grupo | O mesmo, mas a aba continua onde estava e o destino recebe uma cópia. |
| Dividir editor para o grupo à esquerda / direita / acima / abaixo | Copia o editor para o grupo vizinho do focado nessa direção, ou cria um novo grupo ali quando não existe. |
| Novo grupo de editores à esquerda / direita / acima / abaixo | Divide um novo grupo desse lado com uma cópia do editor ativo. Grupos não podem ficar vazios, então o novo grupo sempre começa com o editor ativo. |

## Abas de um grupo

| Comando | O que faz |
| --- | --- |
| Mover editor para a esquerda / direita | Move a aba ativa uma posição ao longo da barra; para nas pontas. Abas fixadas ficam antes das não fixadas. |
| Mover editor para o início / fim do grupo | Move a aba ativa para a primeira ou última posição. |
| Abrir o primeiro / último editor do grupo | Ativa a primeira ou a última aba. |

## Juntar e fechar

| Comando | O que faz |
| --- | --- |
| Juntar grupo de editores com o próximo | Move todas as abas do grupo focado para o próximo (pulando documentos que ele já mostra) e remove o grupo. |
| Juntar todos os grupos de editores | Reúne todos os grupos no focado, sem abas repetidas. |
| Fechar editores à esquerda | Fecha as abas antes da ativa no grupo focado. |
| Fechar editores nos outros grupos | Fecha todas as abas dos grupos que não são o focado. |
| Fechar todos os grupos de editores | Fecha todas as abas. O último grupo permanece. |
| Fechar o editor em todos os grupos | Fecha o documento ativo em todos os grupos. |
| Focar o primeiro / último / ativo grupo de editores | Move o foco para esse grupo. |

Fechar em lote nunca descarta alterações não salvas: um documento com alterações aberto em uma só aba continua aberto.
