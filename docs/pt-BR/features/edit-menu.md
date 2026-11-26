# Menu Editar

O menu **Editar** da barra de título reúne desfazer/refazer, ações da área de transferência, localização e substituição no editor, busca e substituição no workspace, comentários e expansão Emmet. A navegação por teclado usa as mesmas linhas e estados habilitados do ponteiro e da acessibilidade.

Cada arquivo guarda até 10.000 passos de desfazer e até 64 MiB de texto desfeito; além disso, os passos mais antigos são esquecidos. O último passo sempre pode ser desfeito, por maior que seja.

## Substituição no workspace

**Localizar em Arquivos** pesquisa todas as raízes do workspace. **Substituir em Arquivos** adiciona um campo de substituição; pressione **Tab** para alternar entre os campos e **Ctrl+Enter** para executar **Substituir Tudo**. A lista mostra uma prévia dos resultados. Resultados desatualizados cancelam a operação. Documentos abertos recebem alterações não salvas no editor; arquivos fechados são atualizados pela transação do workspace.

## Comentários e Emmet

As ações de comentário de linha e de bloco usam os marcadores da linguagem ativa. Rust, Python, JavaScript e JSONC têm marcadores nativos. Extensões VSIX habilitadas podem contribuir marcadores por `contributes.languages[].configuration`, usando os campos padrão `comments.lineComment` e `comments.blockComment`. A ação fica desabilitada quando não há marcador configurado. Python não tem marcador nativo de comentário de bloco.

A ação Emmet expande uma abreviação no cursor quando o arquivo ativo e o texto atual são compatíveis. **Tab** mantém o comportamento normal do editor e expande Emmet quando possível.
