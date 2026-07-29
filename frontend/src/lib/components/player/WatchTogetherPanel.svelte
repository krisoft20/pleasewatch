<script lang="ts">
    import { fly, fade } from 'svelte/transition';
    import { cubicOut } from 'svelte/easing';
    import { t } from '$lib/i18n';
    import { watchTogether, type ChatMessage } from '$lib/stores/watchTogether';
    import Icon from '../Icon.svelte';

    let {
        isMobile,
        controlsVisible,
        showSettings,
        showSubtitlePicker,
        showEpisodeDrawer,
        kbOffset,
        showChat = $bindable(false),
        chatInput = $bindable(''),
        chatToasts,
        shareCode,
        showShareModal = $bindable(false),
        chatScrollEl = $bindable(),
        chatInputEl = $bindable(),
        onOpenChat,
        onSendChat,
        onCopyShareLink
    }: {
        isMobile: boolean;
        controlsVisible: boolean;
        showSettings: boolean;
        showSubtitlePicker: boolean;
        showEpisodeDrawer: boolean;
        kbOffset: number;
        showChat: boolean;
        chatInput: string;
        chatToasts: ChatMessage[];
        shareCode: string;
        showShareModal: boolean;
        chatScrollEl?: HTMLDivElement;
        chatInputEl?: HTMLInputElement;
        onOpenChat: () => void;
        onSendChat: () => void;
        onCopyShareLink: () => void;
    } = $props();

    const origin = $derived(typeof window !== 'undefined' ? window.location.origin : '');
</script>

{#if $watchTogether.active && !controlsVisible && !showChat && !showSettings && !showSubtitlePicker && !showEpisodeDrawer}
    <button class="pw-chat-corner" onclick={onOpenChat} aria-label="open chat" type="button">
        <Icon name="chat-round" class="w-5 h-5" />
        {#if chatToasts.length > 0}
            <span class="pw-chat-corner-dot"></span>
        {/if}
    </button>
{/if}

{#if chatToasts.length > 0 && !showChat && !isMobile}
    <div class="absolute bottom-28 sm:bottom-36 right-4 z-[25] flex flex-col gap-1.5 pointer-events-none max-w-[260px]">
        {#each chatToasts as toast (toast.time)}
            <button
                type="button"
                class="pw-chat-toast bg-black/65 backdrop-blur-sm rounded-xl px-3 py-1.5 pointer-events-auto cursor-pointer text-left"
                onclick={onOpenChat}
            >
                <span class="text-green-400 text-xs font-semibold">{toast.name}</span>
                <span class="text-white text-xs ml-1">{toast.message}</span>
            </button>
        {/each}
    </div>
{/if}

{#if showChat}
    {#if isMobile}
        {@const recent = $watchTogether.messages.slice(-4)}
        <div class="fixed left-0 right-0 z-50 px-3 pointer-events-none" style="bottom: {kbOffset + 8}px;">
            {#if recent.length > 0}
                <div class="flex flex-col gap-1 mb-2 pointer-events-none">
                    {#each recent as msg, i (msg.time)}
                        <div
                            class="self-start max-w-[80%] bg-black/55 backdrop-blur-sm rounded-xl px-2.5 py-1 text-[13px] leading-tight"
                            style="opacity: {0.55 + (i / Math.max(recent.length - 1, 1)) * 0.45};"
                        >
                            <span class="text-green-400 font-semibold">{msg.name}</span>
                            <span class="text-white/90 ml-1">{msg.message}</span>
                        </div>
                    {/each}
                </div>
            {/if}
            <div
                class="pointer-events-auto flex items-center gap-2 bg-black/55 backdrop-blur-md rounded-full pl-4 pr-1.5 py-1 ring-1 ring-white/10"
                in:fly={{ y: 20, duration: 200, easing: cubicOut }}
                out:fly={{ y: 20, duration: 150 }}
            >
                <input
                    bind:this={chatInputEl}
                    type="text"
                    bind:value={chatInput}
                    placeholder={$t('player.type_a_message')}
                    class="!flex-1 !bg-transparent !border-0 !p-0 !py-1.5 !text-sm !text-white placeholder:!text-white/40 !ring-0 focus:!ring-0 focus:!outline-none"
                    onkeydown={(e) => {
                        if (e.key === 'Enter') onSendChat();
                    }}
                />
                <button
                    onclick={onSendChat}
                    class="w-8 h-8 bg-green-500 active:bg-green-400 rounded-full flex items-center justify-center flex-shrink-0"
                    aria-label="send"
                >
                    <Icon name="send" class="w-3.5 h-3.5 text-white" />
                </button>
                <button
                    onclick={() => {
                        showChat = false;
                    }}
                    class="w-8 h-8 flex items-center justify-center flex-shrink-0 text-white/70 active:text-white"
                    aria-label="close chat"
                >
                    <Icon name="close" class="w-4 h-4" />
                </button>
            </div>
        </div>
    {:else}
        <button
            type="button"
            aria-label="close chat"
            class="absolute inset-0 z-30 bg-transparent border-0 p-0"
            onclick={() => {
                showChat = false;
            }}
        ></button>
        <div in:fly={{ y: 8, duration: 200, easing: cubicOut }} class="absolute z-40 bottom-24 right-10 w-80">
            <div
                class="bg-black/60 backdrop-blur-2xl rounded-2xl shadow-[0_8px_32px_rgba(0,0,0,0.6)] ring-1 ring-white/10 overflow-hidden flex flex-col max-h-[55vh]"
            >
                <div class="bg-white/5 flex items-center justify-between px-4 pt-3 pb-2.5">
                    <div class="flex items-center gap-2">
                        <Icon name="chat-round" class="w-4 h-4 text-gray-400" />
                        <h3 class="text-white text-[13px] font-semibold">{$t('player.chat')}</h3>
                        <div class="flex items-center gap-1 ml-1">
                            <div class="w-1.5 h-1.5 bg-green-400 rounded-full animate-pulse"></div>
                            <span class="text-green-400 text-[11px]">{$watchTogether.participants}</span>
                        </div>
                    </div>
                    <button
                        onclick={() => {
                            showChat = false;
                        }}
                        class="text-gray-500 hover:text-white transition-colors"
                    >
                        <Icon name="close" class="w-4 h-4" />
                    </button>
                </div>
                <div
                    bind:this={chatScrollEl}
                    class="flex-1 overflow-y-auto px-3 py-2.5 space-y-1 scrollbar-hide"
                    style="min-height: 100px;"
                >
                    {#each $watchTogether.messages as msg (msg.time)}
                        <div class="text-[13px] leading-snug {Date.now() - msg.time > 30000 ? 'opacity-50' : ''}">
                            <span class="text-green-400 font-semibold">{msg.name}</span>
                            <span class="text-white/90 ml-1.5">{msg.message}</span>
                        </div>
                    {/each}
                    {#if $watchTogether.messages.length === 0}
                        <p class="text-gray-500 text-xs text-center py-6">{$t('player.no_messages_yet')}</p>
                    {/if}
                </div>
                <div class="px-2.5 pb-2.5 pt-2 border-t border-white/5">
                    <div class="flex items-center gap-2">
                        <input
                            bind:this={chatInputEl}
                            type="text"
                            bind:value={chatInput}
                            placeholder={$t('player.type_a_message')}
                            class="!flex-1 !bg-white/5 !border-white/10 !rounded-lg !px-3 !py-2 !text-[13px]"
                            onkeydown={(e) => {
                                if (e.key === 'Enter') onSendChat();
                            }}
                        />
                        <button
                            onclick={onSendChat}
                            class="w-8 h-8 bg-green-500 hover:bg-green-400 rounded-lg flex items-center justify-center flex-shrink-0 transition-colors"
                            aria-label="send"
                        >
                            <Icon name="send" class="w-4 h-4 text-white" />
                        </button>
                    </div>
                </div>
            </div>
        </div>
    {/if}
{/if}

{#if showShareModal}
    <div class="absolute inset-0 z-[60] flex items-center justify-center" transition:fade={{ duration: 200 }}>
        <button
            type="button"
            aria-label="close share modal"
            class="absolute inset-0 bg-black/70 border-0 p-0"
            onclick={() => {
                showShareModal = false;
            }}
        ></button>
        <div class="relative bg-gray-900 border border-gray-700 rounded-2xl p-6 max-w-sm mx-4 shadow-2xl">
            <h3 class="text-white text-lg font-bold mb-2">
                {$t('player.watch_together')}
            </h3>
            <p class="text-gray-400 text-sm mb-4">
                {$t('player.share_link_with_friends')}
            </p>
            <div class="flex items-center gap-2 bg-gray-800 rounded-lg p-3 mb-4">
                <span class="text-white text-sm font-mono flex-1 truncate">{origin}/party/{shareCode}</span>
                <button
                    onclick={onCopyShareLink}
                    class="bg-primary-600 hover:bg-primary-500 text-white text-sm px-3 py-1.5 rounded-lg flex-shrink-0 transition-colors"
                >
                    {$t('player.copy')}
                </button>
            </div>
            <div class="flex items-center gap-2 text-sm text-gray-400 mb-4">
                <div class="w-2 h-2 bg-green-400 rounded-full animate-pulse"></div>
                <span>{$watchTogether.participants} watching</span>
            </div>
            <button
                onclick={() => {
                    showShareModal = false;
                }}
                class="w-full bg-gray-800 hover:bg-gray-700 text-white py-2.5 rounded-lg transition-colors text-sm font-medium"
            >
                Done
            </button>
        </div>
    </div>
{/if}
