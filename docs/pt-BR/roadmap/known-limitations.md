# Limitações conhecidas (0.1.0)

O Graphite 0.1.0 é o primeiro lançamento. Estas são as coisas que vale saber antes de usá-lo em trabalho importante.

## Plataformas e instalação

- Apenas Windows e Linux em **x86-64**. Não há versões para macOS, ARM ou 32 bits.
- A maior parte da interface foi verificada no Windows. As versões para Linux compilam e passam nos testes, mas a interface em cada combinação de GPU, X11, Wayland e WebKit teve menos testes.
- O instalador do Windows **não é assinado** (Authenticode), então o SmartScreen pode mostrar um aviso.
- As atualizações dentro do aplicativo só instalam arquivos assinados com a chave de atualização do projeto. Qualquer outro deve ser baixado e instalado à mão. Os pacotes nativos do Linux e o AppImage são atualizados pelo gerenciador de pacotes ou baixando de novo.

## Editor e ferramentas

- **Visualização de diferenças:** o texto não tem destaque de sintaxe, linhas mais longas que a coluna são cortadas e não há rolagem horizontal. As regiões sem alterações podem ser recolhidas, mas não expandidas com um clique.
- **Editor de mesclagem:** o painel de resultado é somente leitura; use **Abrir resultado no editor** para terminar à mão.
- **Notebooks:** só são iniciados kernels locais. SVG, páginas HTML e widgets interativos aparecem como texto ou não aparecem. As saídas e imagens não foram conferidas visualmente em todas as plataformas.
- **Depuração:** testada contra um adaptador simulado, não contra todos os adaptadores de depuração reais.
- **Remoto e túneis:** usam o cliente OpenSSH do sistema.

## Extensões

- O suporte a extensões `.vsix` do VS Code existentes é **parcial**. Extensões que precisam de APIs que o Graphite não oferece não funcionarão ou funcionarão só em parte. Veja [Compatibilidade de extensões](../architecture/extension-compat.md).
- Ditado por voz e os comandos proprietários de conta e cobrança de outros editores estão ausentes de propósito.

## Chat e sincronização

- Os painéis de chat precisam de um provedor de modelo que você configura; o Graphite não inclui um.
- A sincronização de configurações funciona por um Gist privado ou por um repositório privado do GitHub. A opção de repositório pede a permissão ampla `repo` do GitHub, porque o GitHub não oferece uma mais estreita para um token pessoal.

## Paridade de recursos

O Graphite não pretende reproduzir todos os comandos de outros editores. Veja o [inventário de paridade](feature-parity.md) e as [etapas restantes](remaining-stages.md) para o que está feito e o que está planejado.
