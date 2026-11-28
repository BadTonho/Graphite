# Workspace do Framework de UI Vitral

A próxima camada de interface do Graphite (cantos arredondados, sombras, layout Flexbox/Grid, animações, acessibilidade e webviews) está sendo construída como o Vitral, um framework de UI Rust de uso geral. O Vitral fica em um repositório próprio e público, [BadTonho/vitral](https://github.com/BadTonho/vitral), incluído aqui como submódulo git em `framework/`.

As crates se chamam `vitral-*`. O nome só fica reservado no crates.io quando a primeira versão for publicada.

## Estrutura

```
Graphite/
├── Cargo.toml        Workspace do Graphite (exclui framework/)
├── .gitmodules       Aponta framework/ para o repositório do Vitral
├── crates/           Crates do Graphite
└── framework/        Submódulo git: o repositório do Vitral
    ├── Cargo.toml    Workspace próprio, Cargo.lock próprio
    ├── crates/       core, layout, render, animation, a11y, platform, workspace-checks
    ├── docs/en/  docs/pt-BR/
    ├── AGENTS.md  LICENSE-MIT  LICENSE-APACHE  README.md
```

- O `Cargo.toml` da raiz lista `framework` em `exclude`, então o framework nunca herda configurações, versões de dependências ou o lock file do workspace do Graphite.
- O Graphite vai depender das crates do framework por `path = "framework/crates/..."`. Quando o Vitral for publicado no crates.io, isso vira uma dependência por versão e o submódulo é removido.

## Trabalhando com o submódulo

Clone o Graphite com o submódulo, ou baixe-o em um clone existente:

```sh
git clone --recurse-submodules https://github.com/BadTonho/Graphite.git
git submodule update --init framework
```

Sem o submódulo, `framework/` fica vazia: o Graphite continua compilando e passando nos testes, mas as etapas do framework no `graphite-xtask verify` falham.

Para alterar o Vitral:

1. Edite, faça commit e push dentro de `framework/`. É um clone normal do repositório do Vitral, com histórico e `AGENTS.md` próprios.
2. No Graphite, faça commit do novo ponteiro do submódulo (`git add framework`), para que o Graphite registre qual commit do Vitral está usando.

Um commit do Graphite nunca contém arquivos do Vitral, só esse ponteiro.

## Regras

- **Uma direção só:** o Graphite pode depender do framework; o framework nunca depende do Graphite nem de código fora do próprio repositório. O teste `vitral_workspace_is_self_contained` garante isso.
- **Erros:** o framework não grava logs. Ele retorna os erros, e o Graphite os registra por `app::error_log` no nível de severidade adequado.
- **Textos:** o framework não tem traduções. O Graphite passa os textos que exibe.

## Adoção

O Graphite adota o Vitral componente por componente, a partir do estágio 3 do Vitral (componentes básicos; veja o [roadmap do Vitral](https://github.com/BadTonho/vitral/blob/main/docs/pt-BR/roadmap/roadmap.md)). O Graphite é a primeira aplicação real do Vitral, então problemas na API aparecem cedo, enquanto a regra de uso geral do Vitral mantém fora das crates principais o que só um editor precisa. Componentes específicos de editor ficam no Graphite ou em crates opcionais do Vitral.

## Verificações

O framework tem build, lint e testes próprios:

```sh
cargo fmt --all --manifest-path framework/Cargo.toml -- --check
cargo clippy --workspace --all-targets --manifest-path framework/Cargo.toml -- -D warnings
cargo test --workspace --manifest-path framework/Cargo.toml
```

`cargo run -p graphite-xtask -- verify` executa esses comandos depois das verificações do próprio Graphite.

A CI baixa o submódulo no commit do Vitral fixado pelo Graphite e executa as mesmas três verificações. O Vitral também tem uma CI própria, no Windows e no Linux, no repositório dele.
