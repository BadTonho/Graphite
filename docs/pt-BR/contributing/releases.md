# Releases em rascunho e pacotes por plataforma

Enviar uma tag de versão aciona `.github/workflows/release.yml`. Commits e pushes de branches não o executam; a CI existente continua manual. O workflow valida a identidade, **cria um draft**, gera os pacotes e anexa somente quando todos os builds passam. Nunca publica. Revise o rascunho e publique manualmente em GitHub Releases. A [opção draft do GitHub CLI](https://cli.github.com/manual/gh_release_create) é explícita.

## Cobertura de plataformas

| Plataforma | Ambiente de build | Artefatos |
|---|---|---|
| Windows | Windows Server 2022, MSVC | ZIP portátil, EXE NSIS por usuário |
| Ubuntu | Ubuntu 22.04 | DEB, tar.gz portátil, AppImage |
| Debian | Contêiner Debian 12 | DEB |
| Fedora | Contêiner Fedora 44 | RPM |
| Arch Linux | Contêiner rolling `archlinux:base-devel` | pkg.tar.zst, usando makepkg sem privilégios |

Todos os alvos são **x86-64**, conforme os downloads verificados de Node/Code OSS. Não são gerados pacotes ARM ou 32 bits. Os pacotes Linux nativos são compilados separadamente por distribuição. Dependências DEB são derivadas por `dpkg-shlibdeps`; RPM usa geração automática. Incluem host, entrada de desktop, ícone e licença, instalam em `/usr/lib/graphite` e expõem `graphite` e `graphite-code`. Não substituem o comando `code` de outro editor.

O instalador Windows usa `%LOCALAPPDATA%/Programs/Graphite`, cria atalho no Menu Iniciar e desinstalador por usuário e mantém PATH. Windows precisa de drivers gráficos compatíveis e WebView2 para webviews. O workflow não aplica assinatura Authenticode ao instalador.

O AppImage é gerado em Ubuntu 22.04, seguindo a [orientação de piso AppImage](https://docs.appimage.org/reference/best-practices.html). Inclui dependências vinculadas, recursos GTK e subprocessos WebKit. O linuxdeploy está fixado em `1-alpha-20251107-1` com verificação SHA-256. O empacotamento verifica extração e arquivos obrigatórios sem FUSE/display. Não se garante glibc anterior ou toda GPU/distribuição; interface/WebKit/X11/Wayland continuam exigindo verificação manual.

## Acionar uma release

1. Inclua em commit os fontes desejados, ferramentas de release e `Cargo.lock`. Defina `workspace.package.version` com a versão sem `v`.
2. Após a inicial `0.1.0`, inclua `changelog/<versão>/en.md` e `pt-BR.md` não vazios, sincronizados e com as categorias exigidas. A versão `0.1.0` não tem changelog.
3. Execute o verificador local e `python -m unittest discover -s packaging/release/tests -v` (Python 3.11+).
4. Crie e envie a tag correspondente:

```sh
git tag -a v0.1.0 -m "Graphite 0.1.0"
git push origin v0.1.0
```

As tags usam `vMAJOR.MINOR.PATCH`, opcionalmente com prerelease como `v0.2.0-rc.1`, e devem corresponder exatamente ao Cargo. Tags inválidas ou changelogs ausentes falham antes de criar o draft. Prereleases ativam o indicador no GitHub, permanecendo em rascunho. As versões dos pacotes nativos normalizam separadores conforme o gerenciador.

## Artefatos e assinatura

São exigidos dez pacotes/artefatos de atualização, com tag/plataforma/arquitetura nos nomes. Cada um recebe `<artefato>.sha256`. Uma lista explícita recusa arquivos ausentes ou inesperados antes do upload. Somente os jobs de draft/upload têm permissão de escrita de release. Builds não recebem chave de assinatura.

Opcionalmente configure o secret de Actions **`GRAPHITE_UPDATE_SIGNING_KEY`** com a chave privada PKCS#8 em base64 usada por `update_signer`. A chave pública precisa já estar compilada em `TRUSTED_UPDATE_KEYS`; consulte [assinatura de atualizações](../features/updates-and-issue-reporter.md). O job usa arquivo temporário fora do checkout, remove ao terminar e exclui a variável dos ambientes dos subprocessos de assinatura. Nunca inclua chaves no Git ou nos uploads.

Com o secret, todos os artefatos recebem também `.sig` Ed25519. Sem ele, o draft recebe checksums e um aviso no workflow; o app rejeita atualizações sem assinatura. Revise confiança da chave/assinaturas antes de publicar. Isso é separado de Authenticode ou assinatura de repositórios de distribuições.

Os arquivos dedicados `graphite-update-<tag>-<plataforma>-x64.zip` contêm somente o executável principal. O atualizador os prefere e exclui pacotes nativos, AppImages, tarballs e instaladores. Instale a distribuição completa para atualizar Node/Code OSS/auxiliares. Use o gerenciador nas instalações Linux nativas: não foi adicionado suporte de autoatualização de instalações gerenciadas ou AppImage.

## Falhas, reexecuções e validação

Um build com falha pode deixar o draft incompleto. Confira Actions e reexecute os jobs. Drafts existentes preservam notas editadas manualmente. Os artefatos de build expiram em sete dias; reexecute tudo se os necessários expirarem. Cada upload verifica o draft e recusa modificar releases publicadas. Não publique durante a execução. O workflow nunca define `draft` como false, envia ao AUR ou cria repositórios apt/dnf/pacman.

Os testes offline cobrem regras de versão/changelog, falha de API, operação somente em draft, interrupção de upload, adulteração, limpeza da chave temporária e travessia de caminhos. A validação local no Windows passou: 17 testes das ferramentas, build do workspace e 1.762 testes Rust (0 falhas, 10 ignorados). YAML, sintaxe Bash via Git Bash e sintaxe do auxiliar Windows também passaram. A execução Linux ficou para o mantenedor conforme solicitado. Builds completos de pacotes/NSIS/AppImage e o workflow real não foram executados localmente: NSIS não está disponível e nenhuma tag/release foi enviada. Valide os pacotes e as [verificações manuais de release](manual-verification.md) antes de publicar o primeiro draft.
