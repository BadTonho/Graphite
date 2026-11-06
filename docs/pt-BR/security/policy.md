# Política de Segurança

## Política de vazamento zero

Nunca faça commit ou upload de: `.env` / `.env.*`, chaves de API, tokens, chaves privadas (`*.pem`, `*.key`, `*.p12`, `*.pfx`), credenciais, bancos locais (`*.db`, `*.sqlite`), certificados de assinatura ou segredos de build. O `.gitignore` da raiz cobre esses padrões.

## Antes de todo commit ou push

1. `git status` — apenas os arquivos pretendidos estão staged.
2. `git diff --staged` — sem segredos ou credenciais.
3. Confirme que padrões sensíveis são ignorados (`git check-ignore <arquivo>`).

## Dependências e licenças

Audite a árvore de dependências antes de releases (`cargo deny` / `cargo license`). Não copie código GPL (por exemplo, do editor Zed) para este projeto MIT.

## Extensões (futuro)

Extensões rodam em um processo Node separado, e plugins WASM serão isolados em sandbox. Extensões são código não confiável: exigirão instalação explícita e limites de permissão claros.

