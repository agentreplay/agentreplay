
import React, { useState, useEffect } from 'react';
import { flowtraceClient, EvalDataset, EvalExample } from '../../src/lib/flowtrace-api';
import { X, Plus, Loader2, CheckCircle, AlertCircle, Database } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';

interface AddToDatasetModalProps {
    isOpen: boolean;
    onClose: () => void;
    initialInput: string;
    initialOutput: string;
    metadata?: Record<string, string>;
}

export function AddToDatasetModal({ isOpen, onClose, initialInput, initialOutput, metadata }: AddToDatasetModalProps) {
    const [datasets, setDatasets] = useState<EvalDataset[]>([]);
    const [loadingDatasets, setLoadingDatasets] = useState(false);
    const [selectedDatasetId, setSelectedDatasetId] = useState<string>('');
    const [createMode, setCreateMode] = useState(false);
    const [newDatasetName, setNewDatasetName] = useState('');

    // Form State
    const [input, setInput] = useState(initialInput);
    const [output, setOutput] = useState(initialOutput);
    const [meta, setMeta] = useState<{ key: string; value: string }[]>(
        metadata ? Object.entries(metadata).map(([k, v]) => ({ key: k, value: String(v) })) : []
    );

    const [isSubmitting, setIsSubmitting] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [success, setSuccess] = useState(false);

    useEffect(() => {
        if (isOpen) {
            fetchDatasets();
            // Reset form when opening
            setInput(initialInput);
            setOutput(initialOutput);
            setMeta(metadata ? Object.entries(metadata).map(([k, v]) => ({ key: k, value: String(v) })) : []);
            setSuccess(false);
            setError(null);
            setCreateMode(false);
        }
    }, [isOpen, initialInput, initialOutput]);

    const fetchDatasets = async () => {
        try {
            setLoadingDatasets(true);
            const response = await flowtraceClient.listDatasets();
            setDatasets(response.datasets || []);
            if (response.datasets?.length > 0 && !selectedDatasetId) {
                // Pre-select first dataset if none selected
                setSelectedDatasetId((response.datasets[0] as any).dataset_id || (response.datasets[0] as any).id);
            }
        } catch (err) {
            console.error('Failed to load datasets:', err);
            setError('Failed to load existing datasets');
        } finally {
            setLoadingDatasets(false);
        }
    };

    const handleSubmit = async () => {
        if (!input.trim()) {
            setError('Input is required');
            return;
        }

        setIsSubmitting(true);
        setError(null);

        try {
            let targetDatasetId = selectedDatasetId;

            // If creating new dataset
            if (createMode) {
                if (!newDatasetName.trim()) {
                    throw new Error('Dataset name is required');
                }
                const newDataset = await flowtraceClient.createDataset(newDatasetName.trim());
                targetDatasetId = newDataset.dataset_id;
            } else if (!targetDatasetId) {
                throw new Error('Please select a dataset');
            }

            // Create Example
            const exampleId = `ex-${Date.now()}`; // Or let server generate? Server usually expects ID in addExamples
            const examples: EvalExample[] = [{
                example_id: exampleId,
                input: input,
                expected_output: output || undefined,
                metadata: meta.reduce((acc, m) => {
                    if (m.key.trim()) acc[m.key.trim()] = m.value.trim();
                    return acc;
                }, {} as Record<string, string>)
            }];

            await flowtraceClient.addExamples(targetDatasetId, examples);

            setSuccess(true);
            setTimeout(() => {
                onClose();
            }, 1500);

        } catch (err: any) {
            console.error('Failed to save to dataset:', err);
            setError(err.message || 'Failed to save example');
        } finally {
            setIsSubmitting(false);
        }
    };

    if (!isOpen) return null;

    return (
        <div className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-[100]" onClick={onClose}>
            <motion.div
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0, scale: 0.95 }}
                className="bg-surface border border-border rounded-xl shadow-2xl w-full max-w-lg overflow-hidden flex flex-col max-h-[90vh]"
                onClick={e => e.stopPropagation()}
            >
                {/* Header */}
                <div className="px-6 py-4 border-b border-border flex items-center justify-between bg-surface-elevated">
                    <h3 className="font-semibold text-lg text-textPrimary flex items-center gap-2">
                        <Database className="w-5 h-5 text-primary" />
                        Add to Evaluation Dataset
                    </h3>
                    <button onClick={onClose} className="text-textSecondary hover:text-textPrimary transition-colors">
                        <X className="w-5 h-5" />
                    </button>
                </div>

                {/* Body */}
                <div className="p-6 overflow-y-auto flex-1 space-y-5">
                    {success ? (
                        <div className="flex flex-col items-center justify-center py-8 text-center">
                            <div className="w-12 h-12 bg-success/10 rounded-full flex items-center justify-center mb-3">
                                <CheckCircle className="w-6 h-6 text-success" />
                            </div>
                            <h4 className="text-lg font-medium text-textPrimary">Saved to Dataset!</h4>
                            <p className="text-textSecondary text-sm mt-1">Ready for evaluation runs.</p>
                        </div>
                    ) : (
                        <>
                            {/* Dataset Selection */}
                            <div className="space-y-2">
                                <div className="flex justify-between items-center">
                                    <label className="text-sm font-medium text-textSecondary">Target Dataset</label>
                                    <button
                                        onClick={() => {
                                            setCreateMode(!createMode);
                                            // Reset name if exiting mode
                                            if (createMode) setNewDatasetName('');
                                        }}
                                        className="text-xs text-primary hover:underline flex items-center gap-1"
                                    >
                                        {createMode ? 'Select Existing' : '+ Create New'}
                                    </button>
                                </div>

                                {createMode ? (
                                    <input
                                        type="text"
                                        value={newDatasetName}
                                        onChange={(e) => setNewDatasetName(e.target.value)}
                                        placeholder="New Dataset Name..."
                                        className="w-full px-3 py-2 bg-background border border-border rounded-lg text-textPrimary text-sm focus:ring-2 focus:ring-primary/20 focus:border-primary outline-none transition-all"
                                        autoFocus
                                    />
                                ) : (
                                    <div className="relative">
                                        {loadingDatasets ? (
                                            <div className="w-full px-3 py-2 bg-muted/20 border border-border rounded-lg text-textSecondary text-sm flex items-center gap-2">
                                                <Loader2 className="w-4 h-4 animate-spin" /> Loading datasets...
                                            </div>
                                        ) : datasets.length === 0 ? (
                                            <div className="w-full px-3 py-2 bg-muted/20 border border-border rounded-lg text-textSecondary text-sm italic">
                                                No datasets found. Create one above.
                                            </div>
                                        ) : (
                                            <select
                                                value={selectedDatasetId}
                                                onChange={(e) => setSelectedDatasetId(e.target.value)}
                                                className="w-full px-3 py-2 bg-background border border-border rounded-lg text-textPrimary text-sm focus:ring-2 focus:ring-primary/20 focus:border-primary outline-none transition-all appearance-none cursor-pointer"
                                            >
                                                {datasets.map((ds: any) => (
                                                    <option key={ds.dataset_id || ds.id} value={ds.dataset_id || ds.id}>
                                                        {ds.name} ({ds.size || ds.examples?.length || 0} examples)
                                                    </option>
                                                ))}
                                            </select>
                                        )}
                                    </div>
                                )}
                            </div>

                            {/* Input / Output Editors */}
                            <div className="space-y-3">
                                <div>
                                    <label className="text-sm font-medium text-textSecondary mb-1 block">Input (Prompt)</label>
                                    <textarea
                                        value={input}
                                        onChange={(e) => setInput(e.target.value)}
                                        className="w-full h-24 px-3 py-2 bg-background border border-border rounded-lg text-textPrimary text-sm font-mono focus:ring-2 focus:ring-primary/20 focus:border-primary outline-none transition-all resize-y"
                                        placeholder="User prompt or input JSON..."
                                    />
                                </div>
                                <div>
                                    <label className="text-sm font-medium text-textSecondary mb-1 block">Expected Output (Ground Truth)</label>
                                    <textarea
                                        value={output}
                                        onChange={(e) => setOutput(e.target.value)}
                                        className="w-full h-24 px-3 py-2 bg-background border border-border rounded-lg text-textPrimary text-sm font-mono focus:ring-2 focus:ring-primary/20 focus:border-primary outline-none transition-all resize-y"
                                        placeholder="Ideal model response..."
                                    />
                                </div>
                            </div>

                            {/* Metadata */}
                            <div>
                                <div className="flex justify-between items-center mb-2">
                                    <label className="text-sm font-medium text-textSecondary">Metadata</label>
                                    <button
                                        onClick={() => setMeta([...meta, { key: '', value: '' }])}
                                        className="text-xs bg-muted px-2 py-1 rounded hover:bg-muted-hover text-textPrimary transition-colors flex items-center gap-1"
                                    >
                                        <Plus className="w-3 h-3" /> Add Field
                                    </button>
                                </div>
                                <div className="space-y-2 max-h-32 overflow-y-auto pr-1">
                                    {meta.map((m, idx) => (
                                        <div key={idx} className="flex gap-2">
                                            <input
                                                value={m.key}
                                                onChange={(e) => {
                                                    const newMeta = [...meta];
                                                    newMeta[idx].key = e.target.value;
                                                    setMeta(newMeta);
                                                }}
                                                placeholder="Key"
                                                className="flex-1 min-w-0 px-2 py-1.5 bg-background border border-border rounded text-xs"
                                            />
                                            <input
                                                value={m.value}
                                                onChange={(e) => {
                                                    const newMeta = [...meta];
                                                    newMeta[idx].value = e.target.value;
                                                    setMeta(newMeta);
                                                }}
                                                placeholder="Value"
                                                className="flex-1 min-w-0 px-2 py-1.5 bg-background border border-border rounded text-xs"
                                            />
                                            <button
                                                onClick={() => setMeta(meta.filter((_, i) => i !== idx))}
                                                className="p-1.5 text-textTertiary hover:text-error transition-colors"
                                            >
                                                <X className="w-3.5 h-3.5" />
                                            </button>
                                        </div>
                                    ))}
                                    {meta.length === 0 && (
                                        <p className="text-xs text-textTertiary italic">No metadata tags.</p>
                                    )}
                                </div>
                            </div>
                        </>
                    )}

                    {error && (
                        <div className="flex items-start gap-2 p-3 bg-error/10 border border-error/20 rounded-lg text-error text-sm animate-in fade-in slide-in-from-top-1">
                            <AlertCircle className="w-4 h-4 mt-0.5 flex-shrink-0" />
                            {error}
                        </div>
                    )}
                </div>

                {/* Footer */}
                <div className="px-6 py-4 border-t border-border bg-surface-elevated flex justify-end gap-3">
                    {!success && (
                        <>
                            <button
                                onClick={onClose}
                                disabled={isSubmitting}
                                className="px-4 py-2 rounded-lg text-sm font-medium text-textSecondary hover:text-textPrimary hover:bg-surface-hover transition-colors"
                            >
                                Cancel
                            </button>
                            <button
                                onClick={handleSubmit}
                                disabled={isSubmitting || loadingDatasets}
                                className="px-4 py-2 rounded-lg text-sm font-medium bg-primary text-white hover:bg-primary-hover shadow-sm hover:shadow transition-all flex items-center gap-2 disabled:opacity-70 disabled:cursor-not-allowed"
                            >
                                {isSubmitting && <Loader2 className="w-4 h-4 animate-spin" />}
                                {createMode ? 'Create & Add' : 'Add to Dataset'}
                            </button>
                        </>
                    )}
                </div>
            </motion.div>
        </div>
    );
}
