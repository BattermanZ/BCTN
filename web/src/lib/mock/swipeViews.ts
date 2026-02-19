import type { Card, SwipeAction } from '$lib/types';

export type SwipeRecord = {
  card_key: string;
  action: SwipeAction;
  by: 'you' | 'partner';
  at: number;
};

export type MatchRecord = {
  card_key: string;
  at: number;
};

export function computeMatches(swipes: SwipeRecord[]): MatchRecord[] {
  const byCard = new Map<string, { you?: SwipeAction; partner?: SwipeAction; at: number }>();
  for (const s of swipes) {
    const prev = byCard.get(s.card_key) ?? { at: s.at };
    if (s.by === 'you') prev.you = s.action;
    if (s.by === 'partner') prev.partner = s.action;
    prev.at = Math.max(prev.at, s.at);
    byCard.set(s.card_key, prev);
  }

  const matches: MatchRecord[] = [];
  for (const [card_key, v] of byCard.entries()) {
    const youLiked = v.you === 'like' || v.you === 'superlike';
    const partnerLiked = v.partner === 'like' || v.partner === 'superlike';
    if (youLiked && partnerLiked) matches.push({ card_key, at: v.at });
  }

  return matches.sort((a, b) => b.at - a.at);
}

export function filterCardsByKeys(cards: Card[], keys: string[]): Card[] {
  const map = new Map(cards.map((c) => [c.card_key, c] as const));
  return keys.map((k) => map.get(k)).filter((v): v is Card => Boolean(v));
}
