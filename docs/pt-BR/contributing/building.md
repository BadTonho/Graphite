# Compilação e Testes

## Requisitos

- Toolchain Rust estável recente (`rustup`).
- GPU/driver compatível com `wgpu` (DirectX 12 ou Vulkan no Windows; Vulkan ou OpenGL no Linux).
- Linux: pacotes de desenvolvimento de X11/Wayland e fontconfig.
- Um compilador C (as gramáticas do tree-sitter compilam código C): MSVC Build Tools no Windows, `gcc`/`clang` no Linux.

## Comandos

```bash
cargo build --workspace
cargo test --workspace
cargo run -p shoppe-app -- <pasta>
```

## Host de extensões simulado

Compile o workspace antes de executar `cargo run -p shoppe-app -- --simulate-extension-host <pasta>`. O auxiliar Rust deve ficar ao lado do Graphite. É desativado por padrão; Node e Code OSS não são necessários. O comando registrado `shoppe.extensions.restartHost` o reinicia manualmente. Consulte [Serviços e host simulado](../architecture/services-host.md) para limites do protocolo, exemplos de atalhos e testes de processo. Para testes específicos da aplicação que usam o auxiliar, execute antes `cargo build -p shoppe-extension-host`; `cargo test --workspace` compila automaticamente o alvo de integração de processos.

## Convenções

- Crie ou atualize testes Rust para comportamentos novos ou alterados e correções de bugs, incluindo casos de falha relevantes. Isole os dados de teste e evite dependência de arquivos pessoais, serviços de rede e servidores de linguagem externos.
- Execute os testes relevantes e `cargo test --workspace` antes de concluir alterações de código; use `cargo build --workspace` para validar a compilação. Informe verificações que falharam ou não foram executadas, o motivo e a plataforma testada. Alterações apenas de documentação não precisam de novos testes, salvo quando alterarem comportamento executável.
- Os testes nativos são executados localmente no sistema disponível. Verifique manualmente interações de janela ou GPU que não puderem ser automatizadas com confiabilidade e informe o que permanecer pendente.
- Mantenha os crates focados; divida um crate quando ele misturar responsabilidades.
- Atualize a documentação em `docs/en/` **e** o arquivo correspondente em `docs/pt-BR/` na mesma mudança.
- Suporte Windows e Linux; use `#[cfg(...)]` quando código específico de plataforma for inevitável.
- Nunca faça commit de segredos (veja a [política de segurança](../security/policy.md)).

