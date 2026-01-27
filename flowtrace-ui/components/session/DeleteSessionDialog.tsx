import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle,
} from '../ui/dialog';
import { Button } from '../ui/button';
import { AlertTriangle, Trash2 } from 'lucide-react';

interface DeleteSessionDialogProps {
    isOpen: boolean;
    onClose: () => void;
    onConfirm: () => void;
    sessionId: string;
}

export function DeleteSessionDialog({
    isOpen,
    onClose,
    onConfirm,
    sessionId,
}: DeleteSessionDialogProps) {
    return (
        <Dialog open={isOpen} onOpenChange={onClose}>
            <DialogContent>
                <DialogHeader>
                    <div className="flex items-center gap-3 text-red-500 mb-2">
                        <div className="p-2 bg-red-500/10 rounded-full">
                            <AlertTriangle className="w-5 h-5" />
                        </div>
                        <DialogTitle>Delete Session</DialogTitle>
                    </div>
                    <DialogDescription>
                        Are you sure you want to delete session <span className="font-mono text-xs bg-muted px-1 py-0.5 rounded">{sessionId}</span>?
                        <br />
                        This action cannot be undone and all associated traces will be permanently removed.
                    </DialogDescription>
                </DialogHeader>

                <DialogFooter className="gap-2 sm:gap-0">
                    <Button variant="outline" onClick={onClose}>
                        Cancel
                    </Button>
                    <Button
                        variant="destructive"
                        onClick={() => {
                            onConfirm();
                            onClose();
                        }}
                        className="bg-red-500 hover:bg-red-600 text-white gap-2"
                    >
                        <Trash2 className="w-4 h-4" />
                        Delete Session
                    </Button>
                </DialogFooter>
            </DialogContent>
        </Dialog>
    );
}
