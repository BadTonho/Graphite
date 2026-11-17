# Auditoria de lançamento

Verificações para fazer antes de criar a tag de um lançamento, com o resultado da auditoria do **0.1.0** (17/11/2026).

## Segredos e arquivos sensíveis

| Verificação | Como | Resultado no 0.1.0 |
|---|---|---|
| Arquivos sensíveis rastreados | `git ls-files` filtrado por `.env`, `*.pem`, `*.key`, `*.p12`, `*.pfx`, `*.db`, `*.sqlite`, credenciais | Nenhum. Dois arquivos de código coincidem só pelo nome (`semantic_tokens.rs`, `tokenizer.rs`) |
| Arquivos sensíveis já commitados | `git log --all --diff-filter=A --name-only` com o mesmo filtro | Nenhum |
| Padrões de segredo no código | `git grep -E` por tokens do GitHub (`ghp_`, `gho_`, `github_pat_`), chaves da AWS, cabeçalhos de chave privada, formatos de Slack e de chaves de API | Só tokens inventados nos testes do redator de erros |
| Padrões de segredo no histórico | `git log --all -G` pelos mesmos padrões | Só o commit que adicionou esses dados de teste |
| `.gitignore` | `git check-ignore -v .env x.pem x.key x.db` | `.env*`, `*.pem`, `*.key`, `*.p12`, `*.pfx`, `*.db`, `*.sqlite` e `/target` são ignorados |
| Dados pessoais | `git grep` por nomes de usuário e caminhos locais; `git log --format=%ae` | Um caminho de exemplo num teste. Os commits usam o endereço no-reply do GitHub |

Repita essas verificações na revisão do lançamento. Uma ocorrência real deve ser removida do histórico, não só do último commit, e a credencial revogada.

## Licenças

- O `THIRD_PARTY_NOTICES.txt` lista 622 componentes no 0.1.0: 621 crates Rust e o pacote Emmet embutido. Todos declaram licenças permissivas (MIT, Apache-2.0, BSD, ISC, Zlib, Unicode, 0BSD, CC0 e semelhantes), exceto dois crates MPL-2.0 (`option-ext`, `uluru`), usados sem modificação, e o `self_cell`, que é `Apache-2.0 OR GPL-2.0-only` e é usado sob a licença Apache. Nenhum crate só GPL ou AGPL está ligado ao programa.
- Rode de novo `python packaging/release/licenses.py --output-dir <pasta>` depois de mudar dependências e leia a lista de licenças procurando qualquer copyleft.

## Pipeline de lançamento

- O workflow de lançamento só cria um **rascunho**; a publicação é manual.
- Os jobs de compilação não recebem chave de assinatura. Só a etapa de assinatura lê `GRAPHITE_UPDATE_SIGNING_KEY`, de um arquivo temporário que é apagado depois. Nunca faça commit nem envie uma chave privada.
- Sem a chave de assinatura de atualizações, os arquivos recebem só somas de verificação e o aplicativo se recusa a instalá-los como atualização interna.

## Verificações manuais

Gere e execute os instaladores reais em máquinas Windows e Linux limpas, seguindo a [verificação manual](../contributing/manual-verification.md), e confirme que os avisos estão dentro do pacote.
