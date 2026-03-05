import React, { useState } from 'react';
import { X } from 'lucide-react';

export default function TagInput({ value, onChange, placeholder, className }) {
    // value is a comma separated string
    const tags = value ? value.split(',').map(t => t.trim()).filter(Boolean) : [];
    const [inputValue, setInputValue] = useState('');

    const addTag = (tag) => {
        const trimmed = tag.trim();
        if (trimmed && !tags.includes(trimmed)) {
            const newTags = [...tags, trimmed];
            onChange(newTags.join(','));
        }
    };

    const removeTag = (indexToRemove) => {
        const newTags = tags.filter((_, index) => index !== indexToRemove);
        onChange(newTags.join(','));
    };

    const handleKeyDown = (e) => {
        if (e.key === 'Enter' || e.key === ',') {
            e.preventDefault();
            addTag(inputValue);
            setInputValue('');
        } else if (e.key === 'Backspace' && !inputValue && tags.length > 0) {
            removeTag(tags.length - 1);
        }
    };

    return (
        <div className={`flex flex-wrap gap-1.5 items-center min-h-[38px] px-2 py-1.5 ${className}`}>
            {tags.map((tag, index) => (
                <div key={`${tag}-${index}`} className="flex items-center gap-1 bg-accent/20 text-accent px-2 py-0.5 rounded text-xs font-semibold">
                    <span>{tag}</span>
                    <button type="button" onClick={() => removeTag(index)} className="hover:text-text-primary transition-colors focus:outline-none">
                        <X size={12} />
                    </button>
                </div>
            ))}
            <input
                type="text"
                value={inputValue}
                onChange={(e) => setInputValue(e.target.value)}
                onKeyDown={handleKeyDown}
                placeholder={tags.length === 0 ? placeholder : ''}
                className="flex-1 min-w-[60px] bg-transparent outline-none text-sm placeholder:text-text-muted/50 text-text-primary"
                onBlur={() => {
                    if (inputValue.trim()) {
                        addTag(inputValue);
                        setInputValue('');
                    }
                }}
            />
        </div>
    );
}
