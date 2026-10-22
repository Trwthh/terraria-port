using System;
using System.Collections.Generic;
using System.IO;
using Steamworks;
using Terraria.IO;
using Terraria.Social.Base;
using Terraria.Utilities;

namespace Terraria.Social.Steam
{
	public class WorkshopHelper
	{
		public class UGCBased
		{
			public struct SteamWorkshopItem
			{
				public string ContentFolderPath;

				public string Description;

				public string PreviewImagePath;

				public string[] Tags;

				public string Title;

				public ERemoteStoragePublishedFileVisibility? Visibility;
			}

			public class Downloader
			{
				public List<string> ResourcePackPaths { get; private set; }

				public List<string> WorldPaths { get; private set; }

				public Downloader()
				{
					ResourcePackPaths = new List<string>();
					WorldPaths = new List<string>();
				}

				public static Downloader Create()
				{
					return new Downloader();
				}

				public List<string> GetListOfSubscribedItemsPaths()
				{
					PublishedFileId_t[] array = new PublishedFileId_t[SteamUGC.GetNumSubscribedItems()];
					SteamUGC.GetSubscribedItems(array, (uint)array.Length);
					ulong punSizeOnDisk = 0uL;
					string pchFolder = string.Empty;
					uint punTimeStamp = 0u;
					List<string> list = new List<string>();
					PublishedFileId_t[] array2 = array;
					for (int i = 0; i < array2.Length; i++)
					{
						if (SteamUGC.GetItemInstallInfo(array2[i], out punSizeOnDisk, out pchFolder, 1024u, out punTimeStamp))
						{
							list.Add(pchFolder);
						}
					}
					return list;
				}

				public bool Prepare(WorkshopIssueReporter issueReporter)
				{
					return Refresh(issueReporter);
				}

				public bool Refresh(WorkshopIssueReporter issueReporter)
				{
					ResourcePackPaths.Clear();
					WorldPaths.Clear();
					foreach (string listOfSubscribedItemsPath in GetListOfSubscribedItemsPaths())
					{
						if (listOfSubscribedItemsPath == null)
						{
							continue;
						}
						try
						{
							string path = listOfSubscribedItemsPath + Path.DirectorySeparatorChar + "workshop.json";
							if (!File.Exists(path))
							{
								continue;
							}
							string text = AWorkshopEntry.ReadHeader(File.ReadAllText(path));
							if (!(text == "World"))
							{
								if (text == "ResourcePack")
								{
									ResourcePackPaths.Add(listOfSubscribedItemsPath);
								}
							}
							else
							{
								WorldPaths.Add(listOfSubscribedItemsPath);
							}
						}
						catch (Exception exception)
						{
							issueReporter.ReportDownloadProblem("Workshop.ReportIssue_FailedToLoadSubscribedFile", listOfSubscribedItemsPath, exception);
							return false;
						}
					}
					return true;
				}
			}

			public class PublishedItemsFinder
			{
				private Dictionary<ulong, SteamWorkshopItem> _items = new Dictionary<ulong, SteamWorkshopItem>();

				private UGCQueryHandle_t m_UGCQueryHandle;

				private CallResult<SteamUGCQueryCompleted_t> OnSteamUGCQueryCompletedCallResult;

				private CallResult<SteamUGCRequestUGCDetailsResult_t> OnSteamUGCRequestUGCDetailsResultCallResult;

				public bool HasItemOfId(ulong id)
				{
					return _items.ContainsKey(id);
				}

				public static PublishedItemsFinder Create()
				{
					PublishedItemsFinder publishedItemsFinder = new PublishedItemsFinder();
					publishedItemsFinder.LoadHooks();
					return publishedItemsFinder;
				}

				private void LoadHooks()
				{
					OnSteamUGCQueryCompletedCallResult = CallResult<SteamUGCQueryCompleted_t>.Create(OnSteamUGCQueryCompleted);
					OnSteamUGCRequestUGCDetailsResultCallResult = CallResult<SteamUGCRequestUGCDetailsResult_t>.Create(OnSteamUGCRequestUGCDetailsResult);
				}

				public void Prepare()
				{
					Refresh();
				}

				public void Refresh()
				{
					m_UGCQueryHandle = SteamUGC.CreateQueryUserUGCRequest(SteamUser.GetSteamID().GetAccountID(), EUserUGCList.k_EUserUGCList_Published, EUGCMatchingUGCType.k_EUGCMatchingUGCType_All, EUserUGCListSortOrder.k_EUserUGCListSortOrder_CreationOrderDesc, SteamUtils.GetAppID(), SteamUtils.GetAppID(), 1u);
					SteamAPICall_t hAPICall = SteamUGC.SendQueryUGCRequest(m_UGCQueryHandle);
					OnSteamUGCQueryCompletedCallResult.Set(hAPICall, OnSteamUGCQueryCompleted);
				}

				private void OnSteamUGCQueryCompleted(SteamUGCQueryCompleted_t pCallback, bool bIOFailure)
				{
					_items.Clear();
					if (bIOFailure || pCallback.m_eResult != EResult.k_EResultOK)
					{
						SteamUGC.ReleaseQueryUGCRequest(m_UGCQueryHandle);
						return;
					}
					for (uint num = 0u; num < pCallback.m_unNumResultsReturned; num++)
					{
						SteamUGC.GetQueryUGCResult(m_UGCQueryHandle, num, out var pDetails);
						ulong publishedFileId = pDetails.m_nPublishedFileId.m_PublishedFileId;
						SteamWorkshopItem value = new SteamWorkshopItem
						{
							Title = pDetails.m_rgchTitle,
							Description = pDetails.m_rgchDescription
						};
						_items.Add(publishedFileId, value);
					}
					SteamUGC.ReleaseQueryUGCRequest(m_UGCQueryHandle);
				}

				private void OnSteamUGCRequestUGCDetailsResult(SteamUGCRequestUGCDetailsResult_t pCallback, bool bIOFailure)
				{
				}
			}

			public abstract class APublisherInstance
			{
				public delegate void FinishedPublishingAction(APublisherInstance instance);

				protected WorkshopItemPublicSettingId _publicity;

				protected SteamWorkshopItem _entryData;

				protected PublishedFileId_t _publishedFileID;

				private UGCUpdateHandle_t _updateHandle;

				private CallResult<CreateItemResult_t> _createItemHook;

				private CallResult<SubmitItemUpdateResult_t> _updateItemHook;

				private FinishedPublishingAction _endAction;

				private WorkshopIssueReporter _issueReporter;

				public void PublishContent(PublishedItemsFinder finder, WorkshopIssueReporter issueReporter, FinishedPublishingAction endAction, string itemTitle, string itemDescription, string contentFolderPath, string previewImagePath, WorkshopItemPublicSettingId publicity, string[] tags)
				{
					_issueReporter = issueReporter;
					_endAction = endAction;
					_createItemHook = CallResult<CreateItemResult_t>.Create(CreateItemResult);
					_updateItemHook = CallResult<SubmitItemUpdateResult_t>.Create(UpdateItemResult);
					ERemoteStoragePublishedFileVisibility visibility = GetVisibility(publicity);
					_entryData = new SteamWorkshopItem
					{
						Title = itemTitle,
						Description = itemDescription,
						ContentFolderPath = contentFolderPath,
						Tags = tags,
						PreviewImagePath = previewImagePath,
						Visibility = visibility
					};
					ulong? num = null;
					if (AWorkshopEntry.TryReadingManifest(contentFolderPath + Path.DirectorySeparatorChar + "workshop.json", out var info))
					{
						num = info.workshopEntryId;
					}
					if (num.HasValue && finder.HasItemOfId(num.Value))
					{
						_publishedFileID = new PublishedFileId_t(num.Value);
						PreventUpdatingCertainThings();
						UpdateItem();
					}
					else
					{
						CreateItem();
					}
				}

				private void PreventUpdatingCertainThings()
				{
					_entryData.Title = null;
					_entryData.Description = null;
				}

				private ERemoteStoragePublishedFileVisibility GetVisibility(WorkshopItemPublicSettingId publicityId)
				{
					switch (publicityId)
					{
					default:
						return ERemoteStoragePublishedFileVisibility.k_ERemoteStoragePublishedFileVisibilityPrivate;
					case WorkshopItemPublicSettingId.FriendsOnly:
						return ERemoteStoragePublishedFileVisibility.k_ERemoteStoragePublishedFileVisibilityFriendsOnly;
					case WorkshopItemPublicSettingId.Public:
						return ERemoteStoragePublishedFileVisibility.k_ERemoteStoragePublishedFileVisibilityPublic;
					}
				}

				private void CreateItem()
				{
					SteamAPICall_t hAPICall = SteamUGC.CreateItem(SteamUtils.GetAppID(), EWorkshopFileType.k_EWorkshopFileTypeFirst);
					_createItemHook.Set(hAPICall, CreateItemResult);
				}

				private void CreateItemResult(CreateItemResult_t param, bool bIOFailure)
				{
					if (bIOFailure)
					{
						param.m_eResult = EResult.k_EResultIOFailure;
					}
					if (param.m_eResult != EResult.k_EResultOK)
					{
						_issueReporter.ReportDelayedUploadProblem("Workshop.ReportIssue_FailedToPublish_WithoutKnownReason", new
						{
							Reason = param.m_eResult.ToString()
						});
						_endAction(this);
						return;
					}
					_publishedFileID = param.m_nPublishedFileId;
					if (!TryWritingManifestToFolder(_entryData.ContentFolderPath, GetHeaderText()))
					{
						_endAction(this);
					}
					else if (param.m_bUserNeedsToAcceptWorkshopLegalAgreement)
					{
						_issueReporter.ReportDelayedUploadProblem("Workshop.ReportIssue_FailedToPublish_UserDidNotAcceptWorkshopTermsOfService");
						_endAction(this);
					}
					else
					{
						UpdateItem();
					}
				}

				protected abstract string GetHeaderText();

				protected abstract void PrepareContentForUpdate();

				private void UpdateItem()
				{
					if (!TryWritingManifestToFolder(_entryData.ContentFolderPath, GetHeaderText()))
					{
						_endAction(this);
						return;
					}
					PrepareContentForUpdate();
					UGCUpdateHandle_t uGCUpdateHandle_t = SteamUGC.StartItemUpdate(SteamUtils.GetAppID(), _publishedFileID);
					if (_entryData.Title != null)
					{
						SteamUGC.SetItemTitle(uGCUpdateHandle_t, _entryData.Title);
					}
					if (_entryData.Description != null)
					{
						SteamUGC.SetItemDescription(uGCUpdateHandle_t, _entryData.Description);
					}
					SteamUGC.SetItemContent(uGCUpdateHandle_t, _entryData.ContentFolderPath);
					SteamUGC.SetItemTags(uGCUpdateHandle_t, _entryData.Tags);
					if (_entryData.PreviewImagePath != null)
					{
						SteamUGC.SetItemPreview(uGCUpdateHandle_t, _entryData.PreviewImagePath);
					}
					if (_entryData.Visibility.HasValue)
					{
						SteamUGC.SetItemVisibility(uGCUpdateHandle_t, _entryData.Visibility.Value);
					}
					SteamAPICall_t hAPICall = SteamUGC.SubmitItemUpdate(uGCUpdateHandle_t, "");
					_updateHandle = uGCUpdateHandle_t;
					_updateItemHook.Set(hAPICall, UpdateItemResult);
				}

				private void UpdateItemResult(SubmitItemUpdateResult_t param, bool bIOFailure)
				{
					if (bIOFailure)
					{
						param.m_eResult = EResult.k_EResultIOFailure;
					}
					switch (param.m_eResult)
					{
					case EResult.k_EResultOK:
						if (param.m_bUserNeedsToAcceptWorkshopLegalAgreement)
						{
							_issueReporter.ReportDelayedUploadProblem("Workshop.ReportIssue_FailedToPublish_UserDidNotAcceptWorkshopTermsOfService");
						}
						SteamFriends.ActivateGameOverlayToWebPage("steam://url/CommunityFilePage/" + _publishedFileID.m_PublishedFileId);
						break;
					default:
						_issueReporter.ReportDelayedUploadProblem("Workshop.ReportIssue_FailedToPublish_WithoutKnownReason", new
						{
							Reason = param.m_eResult.ToString()
						});
						break;
					case EResult.k_EResultAccessDenied:
						_issueReporter.ReportDelayedUploadProblem("Workshop.ReportIssue_FailedToPublish_AccessDeniedBecauseUserDoesntOwnLicenseForApp");
						break;
					case EResult.k_EResultInvalidParam:
						_issueReporter.ReportDelayedUploadProblem("Workshop.ReportIssue_FailedToPublish_InvalidParametersForPublishing");
						break;
					case EResult.k_EResultFileNotFound:
						_issueReporter.ReportDelayedUploadProblem("Workshop.ReportIssue_FailedToPublish_CouldNotFindFolderToUpload");
						break;
					case EResult.k_EResultLockingFailed:
						_issueReporter.ReportDelayedUploadProblem("Workshop.ReportIssue_FailedToPublish_SteamFileLockFailed");
						break;
					case EResult.k_EResultLimitExceeded:
						_issueReporter.ReportDelayedUploadProblem("Workshop.ReportIssue_FailedToPublish_LimitExceeded");
						break;
					}
					_endAction(this);
				}

				private bool TryWritingManifestToFolder(string folderPath, string manifestText)
				{
					string path = folderPath + Path.DirectorySeparatorChar + "workshop.json";
					bool result = true;
					try
					{
						File.WriteAllText(path, manifestText);
					}
					catch (Exception exception)
					{
						_issueReporter.ReportManifestCreationProblem("Workshop.ReportIssue_CouldNotCreateResourcePackManifestFile", exception);
						result = false;
					}
					return result;
				}

				public bool TryGetProgress(out float progress)
				{
					progress = 0f;
					if (_updateHandle == default(UGCUpdateHandle_t))
					{
						return false;
					}
					SteamUGC.GetItemUpdateProgress(_updateHandle, out var punBytesProcessed, out var punBytesTotal);
					if (punBytesTotal == 0L)
					{
						return false;
					}
					progress = (float)((double)punBytesProcessed / (double)punBytesTotal);
					return true;
				}
			}

			public class ResourcePackPublisherInstance : APublisherInstance
			{
				private ResourcePack _resourcePack;

				public ResourcePackPublisherInstance(ResourcePack resourcePack)
				{
					_resourcePack = resourcePack;
				}

				protected override string GetHeaderText()
				{
					return TexturePackWorkshopEntry.GetHeaderTextFor(_resourcePack, _publishedFileID.m_PublishedFileId, _entryData.Tags, _publicity, _entryData.PreviewImagePath);
				}

				protected override void PrepareContentForUpdate()
				{
				}
			}

			public class WorldPublisherInstance : APublisherInstance
			{
				private WorldFileData _world;

				public WorldPublisherInstance(WorldFileData world)
				{
					_world = world;
				}

				protected override string GetHeaderText()
				{
					return WorldWorkshopEntry.GetHeaderTextFor(_world, _publishedFileID.m_PublishedFileId, _entryData.Tags, _publicity, _entryData.PreviewImagePath);
				}

				protected override void PrepareContentForUpdate()
				{
					if (_world.IsCloudSave)
					{
						FileUtilities.CopyToLocal(_world.Path, _entryData.ContentFolderPath + Path.DirectorySeparatorChar + "world.wld");
					}
					else
					{
						FileUtilities.Copy(_world.Path, _entryData.ContentFolderPath + Path.DirectorySeparatorChar + "world.wld", cloud: false);
					}
				}
			}

			public const string ManifestFileName = "workshop.json";
		}
	}
}
